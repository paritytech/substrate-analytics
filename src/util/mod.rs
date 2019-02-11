use crate::db::PurgeLogs;
use ::actix::dev::ToEnvelope;
use ::actix::prelude::*;
use std::time::Duration;

pub struct DbPurge<A: Actor> {
    pub frequency: Duration,
    pub purge_logs: PurgeLogs,
    pub db: Addr<A>,
}

impl<A> Actor for DbPurge<A>
where
    A: Actor + Handler<PurgeLogs>,
    A::Context: ToEnvelope<A, PurgeLogs>,
{
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        ctx.run_interval(self.frequency, |act, _ctx| {
            if let Err(e) = act.db.try_send(act.purge_logs.clone()) {
                error!("Unable to send purge request to DbExecutor: {}", e);
            }
        });
    }
}
