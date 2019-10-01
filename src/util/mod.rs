use actix::prelude::*;
use std::time::Duration;

pub struct PeriodicAction<M: Message<Result = Result<(), &'static str>> + Send> {
    pub interval: Duration,
    pub message: M,
    pub recipient: Recipient<M>,
}

impl<M: Message<Result = Result<(), &'static str>> + Clone + Send + 'static> Actor
    for PeriodicAction<M>
{
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        ctx.run_interval(self.interval, |act, _ctx| {
            if let Err(e) = act.recipient.try_send(act.message.clone()) {
                error!("PeriodicAction: unable to send message to recipient. {}", e);
            }
        });
    }
}
