use async_recursion::async_recursion;
use std::clone::Clone;
use std::sync::Arc;
use tokio::io::{split, AsyncReadExt, AsyncWriteExt, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

const BUF_SIZE: usize = 9999;
const NICK_CMD: &[u8] = b"/nick ";

/// Singly linked list node representing the write half of a connection.
struct LLNode {
    stream: WriteHalf<TcpStream>,
    next: Option<Arc<Mutex<LLNode>>>,
    prev: Option<Arc<Mutex<LLNode>>>,
}

impl LLNode {
    pub fn new(
        stream: WriteHalf<TcpStream>,
        next: Option<Arc<Mutex<LLNode>>>,
        prev: Option<Arc<Mutex<LLNode>>>,
    ) -> LLNode {
        LLNode { stream, next, prev }
    }
}

struct LinkedList {
    /// prev of head is None and next of the tail is None.
    head: Option<Arc<Mutex<LLNode>>>,
}

impl LinkedList {
    /// Returns Arc to Node containing value. This is so we can remove it later.
    pub async fn add(&mut self, value: WriteHalf<TcpStream>) -> Arc<Mutex<LLNode>> {
        let new_node = Arc::new(Mutex::new(LLNode::new(value, self.head.take(), None)));
        if let Some(ref x) = new_node.lock().await.next {
            x.lock().await.prev = Some(new_node.clone());
        };
        self.head = Some(new_node.clone());
        new_node
    }

    /// Removes first encounter of value. Pass in return of add.
    pub async fn remove(&mut self, value: &Arc<Mutex<LLNode>>) {
        if let Some(head) = &self.head {
            let mut current = head.clone();
            loop {
                if Arc::ptr_eq(&current, value) {
                    // Found a match
                    let mut current_lock = current.lock().await;
                    let prev = current_lock.prev.take();
                    let next = current_lock.next.take();
                    drop(current_lock);
                    match &prev {
                        Some(prev) => prev.lock().await.next = next.clone(),
                        // the following should only happen if we are matched with the head.
                        None => self.head = next.clone(),
                    }
                    if let Some(next) = &next {
                        next.lock().await.prev = prev.clone();
                    }
                    return;
                }

                // would be nice to implement this as an async iterator.
                match &current.clone().lock().await.next {
                    Some(next) => current = next.clone(),
                    None => break,
                }
            }
        }
    }

    pub async fn size(&self) -> usize {
        let mut count: usize = 0;
        if let Some(head) = &self.head {
            let mut current = head.clone();
            loop {
                count += 1;

                match &current.clone().lock().await.next {
                    Some(next) => current = next.clone(),
                    None => break,
                }
            }
        }
        count
    }

    pub fn new() -> LinkedList {
        LinkedList { head: None }
    }
}

#[async_recursion]
async fn broadcast(
    node: &mut Arc<Mutex<LLNode>>,
    buf: &[u8],
    nickname: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut lock = node.lock().await;
    {
        lock.stream.write(nickname).await?;
        lock.stream.write(b": ").await?;
        lock.stream.write(buf).await?;
    }
    if let Some(next) = &mut lock.next {
        broadcast(&mut next.clone(), buf, nickname).await?;
    }
    Ok(())
}

#[tokio::main]
// HELP: Why does dyn work but not impl
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting");
    let listener = TcpListener::bind("0.0.0.0:8081").await?;
    let streams = Arc::new(Mutex::new(LinkedList::new()));
    loop {
        let (stream, _) = listener.accept().await?;
        let (mut read_stream, write_stream) = split(stream);

        let streams = streams.clone();
        tokio::spawn(async move {
            let mut nickname: Vec<u8> = Vec::from(b"name".as_slice());

            // we have to explicitly wrap this block in braces, or explicitly drop `stream_ll_head`
            // to drop the lock on the mutex. Otherwise, we can only handle on active connection
            let mut streams_lock = streams.lock().await;
            let my_node = streams_lock.add(write_stream).await;
            println!(
                "There are {} active connections.",
                streams_lock.size().await
            );
            drop(streams_lock);

            // Read write loop
            loop {
                let mut buf: [u8; BUF_SIZE] = [0; BUF_SIZE];

                let n_bytes = match read_stream.read(&mut buf).await {
                    Ok(n) => n,
                    Err(_) => {
                        let mut streams_lock = streams.lock().await;
                        streams_lock.remove(&my_node).await;
                        println!(
                            "There are {} active connections.",
                            streams_lock.size().await
                        );
                        break;
                    }
                };

                if n_bytes == 0 {
                    // the connection probably closed.
                    let mut streams_lock = streams.lock().await;
                    streams_lock.remove(&my_node).await;
                    println!(
                        "There are {} active connections.",
                        streams_lock.size().await
                    );
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
                        if let Some(ref mut node) = &mut streams.lock().await.head {
                            broadcast(node, &buf[..n_bytes], &nickname[..]).await.ok();
                        }
                    }
                }
            }
        });
    }
}
