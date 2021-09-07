use async_std::{prelude::*,stream,task};
use length_prefixed_stream::decode;
use futures::{stream::TryStreamExt};
type Error = Box<dyn std::error::Error+Send+Sync+'static>;

fn main() -> Result<(),Error> {
  task::block_on(async {
    let input = stream::from_iter(vec![
      Ok(vec![6,97,98,99]),
      Ok(vec![100,101]),
      Ok(vec![102,4,65,66]),
      Ok(vec![67,68]),
    ]).into_async_read();
    let mut decoder = decode(input);
    while let Some(chunk) = decoder.next().await {
      println!["{:?}", chunk?];
    }
    Ok(())
  })
}
