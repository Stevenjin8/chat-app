use async_recursion::async_recursion;
use std::clone::Clone;
use std::sync::Arc;
use tokio::io::{split, AsyncReadExt, AsyncWriteExt, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

const BUF_SIZE: usize = 9999;

struct LLNode {
    stream: Arc<Mutex<WriteHalf<TcpStream>>>,
    next: Option<Box<LLNode>>,
}

impl LLNode {
    pub fn new(stream: Arc<Mutex<WriteHalf<TcpStream>>>) -> LLNode {
        LLNode { stream, next: None }
    }

    pub async fn add(self: &mut LLNode, value: Arc<Mutex<WriteHalf<TcpStream>>>) {
        let mut current_node = self;
        loop {
            if current_node.next.is_none() {
                current_node.next = Some(Box::new(LLNode::new(value.clone())));
                return;
            }
            current_node = current_node.next.as_mut().unwrap();
        }
    }
}

#[async_recursion]
async fn broadcast(node: &LLNode, buf: &[u8], nickname: &[u8]) -> () {
    {
        let mut node_lock = node.stream.lock().await;
        node_lock.write(nickname).await.ok();
        node_lock.write(b": ").await.ok();
        node_lock.write(buf).await.ok();
    }
    match &node.next {
        Some(next) => broadcast(next, buf, nickname).await,
        _ => {}
    };
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:8081").await?;
    let stream_ll: Arc<Mutex<Option<LLNode>>> = Arc::new(Mutex::new(Option::None));
    loop {
        let (stream, _) = listener.accept().await?;
        let (mut read_stream, write_stream) = split(stream);

        let stream_ll = stream_ll.clone();
        tokio::spawn(async move {
            let mut nickname: Vec<u8> = Vec::from(b"name".as_slice());
            let write_stream_mutex = Arc::new(Mutex::new(write_stream));

            // we have to explicitly wrap this block in braces, or explictly drop `stream_ll_head`
            // to drop the lock on the mutex. Otherwise, we can only handle on active connection
            {
                let mut stream_ll_head = stream_ll.lock().await;
                if stream_ll_head.is_none() {
                    let value = LLNode::new(write_stream_mutex);
                    *stream_ll_head = Option::Some(value);
                } else {
                    stream_ll_head
                        .as_mut()
                        .unwrap()
                        .add(write_stream_mutex)
                        .await;
                }
            }

            loop {
                let mut buf: [u8; BUF_SIZE] = [0; BUF_SIZE];
                let n_bytes = read_stream.read(&mut buf).await.unwrap();
                if n_bytes == 0 {
                    break;
                }
                match buf.strip_prefix(b"/nick ") {
                    Some(nick) => {
                        nickname.clear();
                        nickname.extend_from_slice(nick);
                        nickname.retain(|x| {*x != b'\n'});
                    }
                    None => {
                        match stream_ll.lock().await.as_ref() {
                            Some(node) => {
                                broadcast(node, &buf[0..n_bytes], &nickname[..]).await;
                            }
                            None => {}
                        };
                    }
                }
            }
        });
    }
}
