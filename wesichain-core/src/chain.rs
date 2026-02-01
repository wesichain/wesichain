use std::marker::PhantomData;

use futures::stream::{self, BoxStream, StreamExt};

use crate::{Runnable, StreamEvent, WesichainError};

pub struct Chain<Head, Tail, Mid> {
    head: Head,
    tail: Tail,
    _marker: PhantomData<Mid>,
}

impl<Head, Tail, Mid> Chain<Head, Tail, Mid> {
    pub fn new(head: Head, tail: Tail) -> Self {
        Self {
            head,
            tail,
            _marker: PhantomData,
        }
    }
}

#[async_trait::async_trait]
impl<Input, Mid, Output, Head, Tail> Runnable<Input, Output> for Chain<Head, Tail, Mid>
where
    Input: Send + 'static,
    Mid: Send + Sync + 'static,
    Output: Send + 'static,
    Head: Runnable<Input, Mid> + Send + Sync,
    Tail: Runnable<Mid, Output> + Send + Sync,
{
    async fn invoke(&self, input: Input) -> Result<Output, WesichainError> {
        let mid = self.head.invoke(input).await?;
        self.tail.invoke(mid).await
    }

    fn stream(&self, input: Input) -> BoxStream<'_, Result<StreamEvent, WesichainError>> {
        type TailStream<'a> = BoxStream<'a, Result<StreamEvent, WesichainError>>;

        enum ChainState<'a, Input> {
            Start(Option<Input>),
            Tail(TailStream<'a>),
            Done,
        }

        let head = &self.head;
        let tail = &self.tail;

        let start_state: ChainState<'_, Input> = ChainState::Start(Some(input));

        stream::unfold(start_state, move |state| async move {
            match state {
                ChainState::Start(Some(input)) => match head.invoke(input).await {
                    Ok(mid) => {
                        let mut tail_stream = tail.stream(mid);
                        match tail_stream.next().await {
                            Some(item) => Some((item, ChainState::Tail(tail_stream))),
                            None => None,
                        }
                    }
                    Err(err) => Some((Err(err), ChainState::Done)),
                },
                ChainState::Tail(mut tail_stream) => match tail_stream.next().await {
                    Some(item) => Some((item, ChainState::Tail(tail_stream))),
                    None => None,
                },
                ChainState::Start(None) | ChainState::Done => None,
            }
        })
        .boxed()
    }
}

pub trait RunnableExt<Input: Send + 'static, Output: Send + 'static>:
    Runnable<Input, Output> + Sized
{
    fn then<NextOutput, Next>(self, next: Next) -> Chain<Self, Next, Output>
    where
        Next: Runnable<Output, NextOutput> + Send + Sync,
        NextOutput: Send + 'static,
    {
        Chain::new(self, next)
    }
}

impl<Input: Send + 'static, Output: Send + 'static, T> RunnableExt<Input, Output> for T where
    T: Runnable<Input, Output> + Sized
{
}
