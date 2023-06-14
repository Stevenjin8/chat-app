# Chat App

Simple tcp chat server in Rust.

## Usage

To run the server, run `cargo build`.
The server will listen on port 8081.
To connect a client, use netcat.

```
nc localhost 8081
```

You can change your nickname with `/nick <new name>`.

```
hello
name: hello
/nick new-name
hi
new-name: hi
```

## Docker

For some reason, the [rust docker images](https://hub.docker.com/_/rust/) are really heavy (over 800MB).
Dockerfile just copies the debug binary and runs it.
That means that before you build the docker image, you have to `cargo build`.
The Makefile provides shortcuts to build and push to a local registry on port 8081. 

## Other Resources

to start local registry
```
docker run -d -p 5000:5000 --restart=always --name registry registry:2
```

create a service principal
https://learn.microsoft.com/en-us/azure/container-registry/container-registry-auth-kubernetes
