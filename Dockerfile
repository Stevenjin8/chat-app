FROM ubuntu:latest
WORKDIR /usr/src/chat-app
EXPOSE 8081
RUN apt update -y
RUN apt upgrade -y
RUN apt install -y net-tools
COPY ./target/debug/hello .
CMD ["./hello"]
