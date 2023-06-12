use async_recursion::async_recursion;
use std::clone::Clone;
use std::sync::Arc;
use tokio::io::{split, AsyncReadExt, AsyncWriteExt, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, MutexGuard};

const BUF_SIZE: usize = 9999;
const NICK_CMD: &[u8] = b"/nick ";

/// Singly linked list node representing the write half of a connection.
struct LLNode {
    /// the actual stream
    stream: WriteHalf<TcpStream>,
    /// the next node. None represents the end of the list.
    next: Option<Box<LLNode>>,
}

impl LLNode {
    /// Create a new node.
    pub fn new(stream: WriteHalf<TcpStream>) -> LLNode {
        LLNode { stream, next: None }
    }
}

#[async_recursion]
async fn broadcast(node: &mut LLNode, buf: &[u8], nickname: &[u8]) -> () {
    {
        // HELP: I want thoughts on my uses of ok/unwrap.
        // Also, kind interesting that this still works even after I've closed connections
        node.stream.write(nickname).await.ok();
        node.stream.write(b": ").await.ok();
        node.stream.write(buf).await.ok();
    }
    if let Some(next) = &mut node.next {
        broadcast(next, buf, nickname).await;
    }
}

/// Add a node to the mutex
async fn add(mut lock: MutexGuard<'_, Option<LLNode>>, value: WriteHalf<TcpStream>) {
    // HELP: What's the point of the anonymous lifetime? Is it just because we need something?
    // Is there a way to do this without calling unwrap?
    // I realize there is insert with and unwrap_or, but I get move errors.
    if let None = *lock {
        *lock = Some(LLNode::new(value));
    } else {
        let mut new_node = Box::new(LLNode::new(value));
        let mut head = lock.take().unwrap();
        new_node.next = head.next;
        head.next = Some(new_node);
        *lock = Some(head);
    }
}

#[tokio::main]
// HELP: Why does dyn work but not impl
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:8081").await?;
    let stream_ll: Arc<Mutex<Option<LLNode>>> = Arc::new(Mutex::new(Option::None));
    loop {
        let (stream, _) = listener.accept().await?;
        let (mut read_stream, write_stream) = split(stream);

        let stream_ll = stream_ll.clone();
        tokio::spawn(async move {
            let mut nickname: Vec<u8> = Vec::from(b"name".as_slice());

            // we have to explicitly wrap this block in braces, or explicitly drop `stream_ll_head`
            // to drop the lock on the mutex. Otherwise, we can only handle on active connection
            {
                let stream_ll_head = stream_ll.lock().await;
                add(stream_ll_head, write_stream).await;
            }

            // Read write loop
            loop {
                let mut buf: [u8; BUF_SIZE] = [0; BUF_SIZE];
                let n_bytes = read_stream.read(&mut buf).await.unwrap();
                if n_bytes == 0 {
                    // the connection probably closed. Ideally, we would remove the corresponding
                    // write stream from the linked list of streams
                    break;
                }
                match buf.strip_prefix(NICK_CMD) {
                    Some(nick) => {
                        // The user wants a new nickname
                        // HELP: Still not feeling too confident on string/byte manipulation.
                        nickname.clear();
                        nickname.extend_from_slice(nick);
                        // HELP: feels like there should be a better way to do this. I tried having
                        // b'\n'.eq be the argument, but that didn't work.
                        nickname.retain(|x| *x != b'\n');
                        // I hate that the formatter removes the braces
                    }
                    None => {
                    // broadcast the message
                        if let Some(node) = stream_ll.lock().await.as_mut() {
                            broadcast(node, &buf[0..n_bytes], &nickname[..]).await;
                        }
                    }
                }
            }
        });
    }
}
