# Developing Fluvio

Thank you for joining Fluvio community. The goal of this document is to provide everything you need to get started with developing Fluvio.

## Assumptions

Familiarity with

- [Rust](https://www.rust-lang.org)
- [Kubernetes](https://kubernetes.io)

This Developer's guide and examples should work with the following platforms:

- MacOS X
- Linux  

Other platforms such as Windows can be made to work, but we haven't tried them yet.

To test and run services, you need to get access to development Kubernetes cluster.
Our guide uses Minikube as examples because it is easy to it get it started,
but you can use another Kubernetes cluster as well.
Please see [Kubernetes](https://kubernetes.io) for setting up a development cluster.

Please read [doc](www.fluvio.io) for technical arch and operation guide.

# Setting up Development Environment

## Set up Rust

Please follow [setup](https://www.rust-lang.org/tools/install) instructions to install Rust and Cargo.

## Install minikube

Please follow [minikube](https://minikube.sigs.k8s.io/docs/start/) to install minikube.

## Install Helm

Please follow [helm setup](https://helm.sh/docs/intro/quickstart/) to install hel

## Checkout and build Fluvio

Build Fluvio CLI from source code

```
$ git clone https://github.com/infinyon/fluvio.git
$ cd fluvio
$ cargo build
```

The next step is very important, as it will help you to prevent subtle development
bugs. Fluvio is built in two separate pieces, `fluvio` (the CLI), and `fluvio-run`
(the server). When testing changes to these components, you need to make sure to
rebuild _both_ components before running. In other Rust projects, it is typical to
just use `cargo run`:

```
$ cargo run -- my CLI args here
```

However, this will only rebuild `fluvio`, it will not also rebuild `fluvio-run`,
which may make you think that the code changes you made did not have any effect.
In order to automate the rebuilding of both of these components, we STRONGLY
RECOMMEND adding the following alias to your `~/.bashrc` or `~/.zshrc` file:

```
alias flvd='cargo build --manifest-path="/Users/nick/infinyon/fluvio/Cargo.toml" --bin fluvio-run && \
    cargo run --manifest-path="/Users/nick/infinyon/fluvio/Cargo.toml" --bin fluvio --'
```

Make sure to replace `/Users/nick/infinyon/fluvio` with the path where you cloned `fluvio`
on your own system. Then, the `flvd` command (short for "fluvio develop") will recompile
both `fluvio-run` and `fluvio`, then execute `fluvio` and pass the arguments to it.

## Download a published version of Fluvio

If, instead of building Fluvio, you would prefer to just download it and get to work,
you can use our one-line installation script. You can use it to install the latest
release or prerelease, or to install a specific version:

```
$ curl -fsS https://packages.fluvio.io/v1/install.sh | bash                 # Install latest release
$ curl -fsS https://packages.fluvio.io/v1/install.sh | VERSION=latest bash  # Install latest pre-release
$ curl -fsS https://packages.fluvio.io/v1/install.sh | VERSION=x.y.z bash   # Install specific version
```

## Setting Kubernetes up for running Fluvio in development

Install Fluvio `sys` chart from source.

```
$ flvd cluster start --sys --develop
```

# Running Fluvio with local cluster

We highly recommend using the `flvd cluster start --local --develop` command for most development.

However, in the following cases, we run `sc` and `spu` individually, allowing individual testing.

## Starting SC

The Streaming Controller (SC) is the controller for a Fluvio cluster.
You only start a single SC for a single Fluvio cluster.

To run the SC, you'll need to build and run the `fluvio-run` executable:

```
$ RUST_LOG=fluvio=debug cargo run --bin fluvio-run -- sc
```

## Starting SPU

After SC is started, you can start adding unmanaged (custom) SPUs.

For each SPU, first register the SPU. For example, the following registers a SPU with ID 5001 with public and private ports. 
Normally, you only need to register a SPU once.

```
$ flvd cluster spu register --id 5001 --public-server 0.0.0.0:9010 --private-server  0.0.0.0:9011
```

Then you can start SPU 5001

```
$ cargo run --bin fluvio-run -- spu -i 5001 -p 0.0.0.0:9010 -v 0.0.0.0:9011 > /tmp/spu_5001.log
```

The logs can be found in `/tmp/spu_5001.log`.

Now, you should see SPU being active:

```
$ flvd cluster spu list
 ID    NAME             STATUS  TYPE      RACK  PUBLIC        PRIVATE 
 5001  custom-spu-5001  Online  "custom"   -    0.0.0.0:9010  0.0.0.0:9011 
```

Can create new topic
```
$ flvd topic create topic
topic "topic" created
```

Produce and consume works:
```
$ flvd produce topic
hello world
Ok!

$ flvd consume topic -B -d
hello world
```

You can launch additional SPU as needed, just ensure that ports doesn't conflict with each other.
For example, to add 2nd:

```
$ flvd cluster spu register --id 5001 --public-server 0.0.0.0:9020 --private-server  0.0.0.0:9021
$ cargo run --bin fluvio-run -- spu -i 5001 -p 0.0.0.0:9020 -v 0.0.0.0:9021
```


# Compiling for K8

In order to deploy to minikube.  Docker image version must be built.

Fluvio uses [musl](https://musl.libc.org) for deploying on a docker image.  


## Setting up target
First, install Rust target:

```
rustup target add x86_64-unknown-linux-musl
```

For mac:

```
brew install filosottile/musl-cross/musl-cross
export TARGET_CC=x86_64-linux-musl-gcc
```

For Linux, please see [musl wiki](https://wiki.musl-libc.org) for the installation of musl-gcc.

For ubuntu:

```
sudo apt install -y musl-tools
export TARGET_CC=musl-gcc
sudo ln -s /usr/bin/musl-gcc /usr/local/bin/x86_64-linux-musl-gcc
```

## To build docker image

Run following command to build image

```
$ make minikube_image
```

Make sure you uninstall previous clusters for local and k8:

```
$ flvd cluster delete --local
$ flvd cluster delete
```

Run command below now to run install with image just built

```
$ flvd cluster start --develop
```

Topic creation, product and consumer can now be tested as with `local` cluster.

You can remove fluvio cluster by

```
$ flvd cluster delete
```

Note that when you uninstall cluster, CLI will remove all related objects such as

- Topics
- Partitions
- Tls Secrets
- Storage

## Running SC in locally

First install fluvio k8 cluster as normally.

Then delete deployment:

```
kubectl delete deployment fluvio-sc
```

Then, can run sc directly

```
cargo run --bin fluvio-sc-k8
```

## Troubleshooting

This guide helps users to solve issues they might face during the setup process. 

###### Cross-compilation errors

If you face cross-compilation errors while creating minikube image, for example

```
cargo build --bin fluvio-spu --target x86_64-unknown-linux-musl
error: linker `x86_64-linux-musl-gcc` not found
 |
 = note: No such file or directory (os error 2)
error: aborting due to previous error
error: could not compile `fluvio-spu`.
```

This is indicative that you need to add standard library for the target platform:

```
rustup target add x86_64-unknown-linux-musl
```

If it still doesn't work

```
brew install filosottile/musl-cross/musl-cross
```

Make sure you set the following environment variable

```
export TARGET_CC=x86_64-linux-musl-gcc
```

###### Connection issues

If you face issues while connecting to the registry

```
Get http://localhost:5000/v2/: dial tcp [::1]:5000: connect: connection refused
```

It means your docker registry is not running

```
docker run -d -p 5000:5000 --restart=always --name registry registry:2
```

If you face connection issues while creating minikube image while your docker registry is up running

```
$ make minikube_image
Error response from daemon: Get http://localhost:5000/v2/: dial tcp 127.0.0.1:5000: connect: connection refused
make[1]: *** [minikube] Error 1
make: *** [spu_image] Error 2
```
Re-build i.e.delete and restart minikube cluster

```
sh k8-util/minikube/reset-minikube.sh
```


###### Fluvio sys chart issues

If you face issues while installing sys chart

```
$ flvd cluster start --sys
"fluvio" has been added to your repositories
Hang tight while we grab the latest from your chart repositories...
...Successfully got an update from the "fluvio" chart repository
Update Complete. ⎈ Happy Helming!⎈ 
Exited with status code: 1
thread 'main' panicked at 'assertion failed: false', src/cli/src/cluster/util.rs:115:17
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

Rebuilding minikube cluster sometimes doesnt remove the storage class. Hence the sys chart installation throws an error. Make sure the storage class is deleted.

```
kubectl delete storageclass fluvio-spu
```

#### Deleting partition

In certain cases, partition may not be deleted correctly.  In this case, you can manually force delete by:
```
kubectl patch partition  <partition_name> -p '{"metadata":{"finalizers":null}}' --type merge
```
