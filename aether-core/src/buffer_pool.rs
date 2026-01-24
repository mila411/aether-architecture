//! Buffer pool for reducing allocations.

use bytes::BytesMut;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Debug)]
pub struct BytePool {
    inner: Arc<Mutex<Vec<BytesMut>>>,
    buffer_capacity: usize,
    max_buffers: usize,
}

impl BytePool {
    pub fn new(buffer_capacity: usize, max_buffers: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
            buffer_capacity: buffer_capacity.max(1),
            max_buffers: max_buffers.max(1),
        }
    }

    pub async fn acquire(&self) -> PooledBytesMut {
        let mut pool = self.inner.lock().await;
        let buffer = pool.pop().unwrap_or_else(|| BytesMut::with_capacity(self.buffer_capacity));
        PooledBytesMut {
            pool: self.clone(),
            buffer: Some(buffer),
        }
    }

    async fn release(&self, mut buffer: BytesMut) {
        buffer.clear();
        let mut pool = self.inner.lock().await;
        if pool.len() < self.max_buffers {
            pool.push(buffer);
        }
    }
}

#[derive(Debug)]
pub struct PooledBytesMut {
    pool: BytePool,
    buffer: Option<BytesMut>,
}

impl PooledBytesMut {
    pub fn as_mut(&mut self) -> &mut BytesMut {
        self.buffer.as_mut().expect("buffer already taken")
    }

    pub fn len(&self) -> usize {
        self.buffer.as_ref().map(|b| b.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub async fn release(mut self) {
        if let Some(buffer) = self.buffer.take() {
            self.pool.release(buffer).await;
        }
    }
}

impl Drop for PooledBytesMut {
    fn drop(&mut self) {
        if let Some(buffer) = self.buffer.take() {
            let pool = self.pool.clone();
            tokio::spawn(async move {
                pool.release(buffer).await;
            });
        }
    }
}
