#![feature(async_closure)]
#![allow(unused_assignments)]
use async_std::{prelude::*,stream::Stream};
use futures::io::AsyncRead;
mod unfold;
use unfold::unfold;
use std::marker::Unpin;
use desert::varint;
use std::collections::VecDeque;
type Error = Box<dyn std::error::Error+Send+Sync+'static>;

pub fn decode(input: impl AsyncRead+Unpin+'static) -> Box<dyn Stream<Item=Result<Vec<u8>,Error>>+Unpin> {
  let state = Decoder::new(input, 10_000);
  Box::new(unfold(state, async move |mut state| {
    match state.next().await {
      Ok(Some(x)) => Some((Ok(x),state)),
      Ok(None) => None,
      Err(e) => Some((Err(e.into()),state)),
    }
  }))
}

struct Decoder<AR: AsyncRead> {
  input: AR,
  buffer: Vec<u8>,
  queue: VecDeque<Vec<u8>>,
  write_offset: usize,
}

impl<AR> Decoder<AR> where AR: AsyncRead+Unpin+'static {
  pub fn new(input: AR, max_size: usize) -> Self {
    Self {
      input,
      buffer: vec![0u8;max_size],
      write_offset: 0,
      queue: VecDeque::new(),
    }
  }
  pub async fn next(&mut self) -> Result<Option<Vec<u8>>,Error> {
    if let Some(buf) = self.queue.pop_front() {
      return Ok(Some(buf));
    }
    let mut msg_len = 0;
    let mut read_offset = 0;
    loop {
      let n = self.input.read(&mut self.buffer[self.write_offset..]).await?;
      if n == 0 && self.write_offset == 0 {
        return Ok(None);
      } else if n == 0 {
        panic!["unexpected end of input stream while decoding varint"];
      }
      self.write_offset += n;
      match varint::decode(&self.buffer) {
        Ok((s,len)) => {
          msg_len = len as usize;
          read_offset = s;
          break;
        },
        Err(e) => {
          if self.write_offset >= 10 {
            return Err(e.into());
          }
        },
      }
    }
    loop {
      if msg_len == 0 { break }
      if msg_len + read_offset > self.write_offset {
        let n = self.input.read(&mut self.buffer[self.write_offset..]).await?;
        if n == 0 {
          panic!["unexpected end of message content"];
        }
        self.write_offset += n;
      } else {
        break;
      }
    }
    let buf = self.buffer[read_offset..read_offset+msg_len].to_vec();
    {
      let mut offset = read_offset + msg_len;
      let mut vlen = 0;
      loop { // push remaining complete records in this buffer to queue
        match varint::decode(&self.buffer[offset..]) {
          Ok((s,len)) => {
            msg_len = len as usize;
            vlen = s;
          },
          _ => { break }
        }
        if msg_len == 0 { break }
        if offset + vlen + msg_len > self.write_offset {
          break;
        }
        offset += vlen;
        let qbuf = self.buffer[offset..offset+msg_len].to_vec();
        self.queue.push_back(qbuf);
        offset += msg_len;
      }
      self.buffer.copy_within(offset.., 0);
      self.write_offset -= offset;
      //self.write_offset = 0;
    }
    Ok(Some(buf))
  }
}
