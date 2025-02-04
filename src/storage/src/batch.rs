use std::io::Error as IoError;
use std::io::Cursor;
use std::io::ErrorKind;
use std::io::SeekFrom;
use std::fmt::Debug;
use std::marker::PhantomData;

use tracing::trace;
use tracing::debug;
use futures_lite::io::AsyncReadExt;
use futures_lite::io::AsyncSeekExt;

use fluvio_future::fs::File;
use dataplane::batch::{
    Batch, BatchRecords, DefaultBatchRecords, BATCH_PREAMBLE_SIZE, BATCH_HEADER_SIZE,
    BATCH_FILE_HEADER_SIZE,
};
use dataplane::Size;
use dataplane::Offset;

use crate::StorageError;

pub type DefaultFileBatchStream = FileBatchStream<DefaultBatchRecords>;

/// hold information about position of batch in the file
pub struct FileBatchPos<R>
where
    R: BatchRecords,
{
    inner: Batch<R>,
    pos: Size,
}

impl<R> Unpin for FileBatchPos<R> where R: BatchRecords {}

#[allow(clippy::len_without_is_empty)]
impl<R> FileBatchPos<R>
where
    R: BatchRecords,
{
    fn new(inner: Batch<R>, pos: Size) -> Self {
        FileBatchPos { inner, pos }
    }

    pub fn get_batch(&self) -> &Batch<R> {
        &self.inner
    }

    pub fn get_pos(&self) -> Size {
        self.pos
    }

    pub fn get_base_offset(&self) -> Offset {
        self.inner.get_base_offset()
    }

    pub fn get_last_offset(&self) -> Offset {
        self.inner.get_last_offset()
    }

    /// batch length (without preamble)
    pub fn len(&self) -> Size {
        self.inner.batch_len as Size
    }

    /// total batch length including preamble
    pub fn total_len(&self) -> Size {
        self.len() + BATCH_PREAMBLE_SIZE as Size
    }

    pub fn records_remainder_bytes(&self, remainder: usize) -> usize {
        self.inner.records().remainder_bytes(remainder)
    }

    /// decode next batch from file
    pub(crate) async fn from(
        file: &mut File,
        pos: Size,
    ) -> Result<Option<FileBatchPos<R>>, IoError> {
        let mut bytes = vec![0u8; BATCH_FILE_HEADER_SIZE];

        let read_len = file.read(&mut bytes).await?;
        trace!(
            "file batch: read preamble and header {} bytes out of {}",
            read_len,
            BATCH_FILE_HEADER_SIZE
        );

        if read_len == 0 {
            trace!("no more bytes,there are no more batches");
            return Ok(None);
        }

        if read_len < BATCH_FILE_HEADER_SIZE {
            return Err(IoError::new(
                ErrorKind::UnexpectedEof,
                format!(
                    "expected: {} but only {} bytes read",
                    BATCH_FILE_HEADER_SIZE, read_len
                ),
            ));
        }

        let mut cursor = Cursor::new(bytes);
        let mut batch = Batch::default();
        batch.decode_from_file_buf(&mut cursor, 0)?;
        let mut file_batch = FileBatchPos::new(batch, pos);

        let remainder = file_batch.len() as usize - BATCH_HEADER_SIZE as usize;
        trace!(
            "file batch: offset: {}, len: {}, total: {}, remainder: {}, pos: {}",
            file_batch.get_batch().get_last_offset_delta(),
            file_batch.len(),
            file_batch.total_len(),
            remainder,
            pos
        );

        if file_batch.records_remainder_bytes(remainder) > 0 {
            trace!("file batch reading records with remainder: {}", remainder);
            file_batch.read_records(file, remainder).await?
        } else {
            trace!("file batch seeking next batch");
            file_batch.seek_to_next_batch(file, remainder).await?;
        }

        Ok(Some(file_batch))
    }

    /// decode the records
    async fn read_records<'a>(
        &'a mut self,
        file: &'a mut File,
        remainder: usize,
    ) -> Result<(), IoError> {
        let mut bytes = vec![0u8; remainder];
        let read_len = file.read(&mut bytes).await?;
        trace!(
            "file batch: read records {} bytes out of {}",
            read_len,
            remainder
        );

        if read_len < remainder {
            return Err(IoError::new(
                ErrorKind::UnexpectedEof,
                "not enough for records",
            ));
        }

        let mut cursor = Cursor::new(bytes);
        self.inner.mut_records().decode(&mut cursor, 0)?;

        Ok(())
    }

    async fn seek_to_next_batch<'a>(
        &'a self,
        file: &'a mut File,
        remainder: usize,
    ) -> Result<(), IoError> {
        if remainder > 0 {
            trace!("file batch skipping: content {} bytes", remainder);
            let seek_position = file.seek(SeekFrom::Current(remainder as Offset)).await?;
            trace!("file batch new position: {}", seek_position);
        }

        Ok(())
    }
}

// stream to iterate batch
pub struct FileBatchStream<R>
where
    R: Default + Debug,
{
    pos: Size,
    invalid: Option<IoError>,
    file: File,
    data: PhantomData<R>,
}

impl<R> FileBatchStream<R>
where
    R: Default + Debug,
{
    #[allow(dead_code)]
    pub fn new(file: File) -> FileBatchStream<R> {
        //trace!("opening batch stream on: {}",file);
        FileBatchStream {
            pos: 0,
            file,
            invalid: None,
            data: PhantomData,
        }
    }

    #[allow(dead_code)]
    pub async fn new_with_pos(
        mut file: File,
        pos: Size,
    ) -> Result<FileBatchStream<R>, StorageError> {
        trace!("opening batch  stream at: {}", pos);
        let seek_position = file.seek(SeekFrom::Start(pos as u64)).await?;
        if seek_position != pos as u64 {
            return Err(IoError::new(ErrorKind::UnexpectedEof, "not enough for position").into());
        }
        Ok(FileBatchStream {
            pos,
            file,
            invalid: None,
            data: PhantomData,
        })
    }

    /// check if it is invalid
    pub fn invalid(self) -> Option<IoError> {
        self.invalid
    }
}

impl<R> FileBatchStream<R>
where
    R: BatchRecords,
{
    pub async fn next(&mut self) -> Option<FileBatchPos<R>> {
        trace!("reading next from pos: {}", self.pos);
        match FileBatchPos::from(&mut self.file, self.pos).await {
            Ok(batch_res) => {
                if let Some(ref batch) = batch_res {
                    trace!("batch founded, updating pos");
                    self.pos += batch.total_len() as Size;
                } else {
                    trace!("no batch founded");
                }
                batch_res
            }
            Err(err) => {
                debug!("error getting batch: {}", err);
                self.invalid = Some(err);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use std::env::temp_dir;
    use std::path::PathBuf;

    use fluvio_future::test_async;
    use flv_util::fixture::ensure_new_dir;
    use dataplane::fixture::create_batch;
    use dataplane::fixture::create_batch_with_producer;

    use crate::config::ConfigOption;
    use crate::StorageError;
    use crate::segment::MutableSegment;

    fn default_option(base_dir: PathBuf) -> ConfigOption {
        ConfigOption {
            base_dir,
            segment_max_bytes: 1000,
            index_max_bytes: 1000,
            ..Default::default()
        }
    }

    #[test_async]
    async fn test_batch_stream_single() -> Result<(), StorageError> {
        let test_dir = temp_dir().join("batch-stream-single");
        ensure_new_dir(&test_dir)?;

        let option = default_option(test_dir.clone());

        let mut active_segment = MutableSegment::create(300, &option).await?;

        active_segment
            .write_batch(&mut create_batch())
            .await
            .expect("writing batches");

        let mut batch_stream = active_segment
            .open_batch_header_stream(0)
            .await
            .expect("open file batch stream");
        let batch1 = batch_stream.next().await.expect("batch");
        let batch = batch1.get_batch();
        assert_eq!(batch.get_base_offset(), 300);
        assert_eq!(batch.get_header().producer_id, 12);
        assert_eq!(batch1.get_last_offset(), 301);
        Ok(())
    }

    #[test_async]
    async fn test_batch_stream_multiple() -> Result<(), StorageError> {
        let test_dir = temp_dir().join("batch-stream");
        ensure_new_dir(&test_dir)?;

        let option = default_option(test_dir.clone());

        let mut active_segment = MutableSegment::create(300, &option).await?;

        active_segment.write_batch(&mut create_batch()).await?;
        active_segment
            .write_batch(&mut create_batch_with_producer(25, 2))
            .await?;

        let mut batch_stream = active_segment
            .open_batch_header_stream(0)
            .await
            .expect("open file batch stream");

        let batch1 = batch_stream.next().await.expect("batch");
        assert_eq!(batch1.get_last_offset(), 301);
        let batch2 = batch_stream.next().await.expect("batch");
        assert_eq!(batch2.get_last_offset(), 303);

        Ok(())
    }
}
