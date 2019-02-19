use ::actix::prelude::*;
use ::actix_web::*;
use std::time::Duration;

pub struct PeriodicAction<M: Message<Result = Result<(), Error>> + Send> {
    pub frequency: Duration,
    pub message: M,
    pub recipient: Recipient<M>,
}

impl<M: Message<Result = Result<(), Error>> + Clone + Send + 'static> Actor for PeriodicAction<M> {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        ctx.run_interval(self.frequency, |act, _ctx| {
            if let Err(e) = act.recipient.try_send(act.message.clone()) {
                error!("Unable to send message to Recipient. {}", e);
            }
        });
    }
}
