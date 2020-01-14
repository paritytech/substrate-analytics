// Copyright 2019 Parity Technologies (UK) Ltd.
// This file is part of Substrate Analytics.

// Substrate Analytics is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate Analytics is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate Analytics.  If not, see <http://www.gnu.org/licenses/>.

use actix::prelude::*;
use std::time::Duration;

pub struct PeriodicAction<M: Message<Result = Result<(), &'static str>> + Send> {
    pub interval: Duration,
    pub message: M,
    pub recipient: Recipient<M>,
}

impl<M: Message<Result = Result<(), &'static str>> + Clone + Send + 'static + Unpin> Actor
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
