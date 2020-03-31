use crate::{
    message::BasicReturnMessage,
    pinky_swear::{PinkyBroadcaster, PinkySwear},
    publisher_confirm::Confirmation,
    BasicProperties,
};
use log::error;
use parking_lot::Mutex;
use std::{collections::VecDeque, sync::Arc};

#[derive(Clone, Debug, Default)]
pub(crate) struct ReturnedMessages {
    inner: Arc<Mutex<Inner>>,
}

impl ReturnedMessages {
    pub(crate) fn start_new_delivery(&self, message: BasicReturnMessage) {
        self.inner.lock().current_message = Some(message);
    }

    pub(crate) fn set_delivery_properties(&self, properties: BasicProperties) {
        if let Some(message) = self.inner.lock().current_message.as_mut() {
            message.delivery.properties = properties;
        }
    }

    pub(crate) fn new_delivery_complete(&self) {
        self.inner.lock().new_delivery_complete();
    }

    pub(crate) fn receive_delivery_content(&self, data: Vec<u8>) {
        if let Some(message) = self.inner.lock().current_message.as_mut() {
            message.delivery.data.extend(data);
        }
    }

    pub(crate) fn drain(&self) -> Vec<BasicReturnMessage> {
        self.inner.lock().drain()
    }

    pub(crate) fn register_pinky(&self, pinky: PinkyBroadcaster<Confirmation>) {
        self.inner.lock().register_pinky(pinky);
    }

    pub(crate) fn register_dropped_confirm(&self, promise: PinkySwear<Confirmation>) {
        self.inner.lock().dropped_confirms.push(promise);
    }
}

#[derive(Debug, Default)]
pub struct Inner {
    current_message: Option<BasicReturnMessage>,
    messages: VecDeque<BasicReturnMessage>,
    dropped_confirms: Vec<PinkySwear<Confirmation>>,
    pinkies: VecDeque<PinkyBroadcaster<Confirmation>>,
}

impl Inner {
    fn register_pinky(&mut self, pinky: PinkyBroadcaster<Confirmation>) {
        if let Some(message) = self.messages.pop_front() {
            pinky.swear(Confirmation::Nack(message));
        } else {
            self.pinkies.push_back(pinky);
        }
    }

    fn new_delivery_complete(&mut self) {
        if let Some(message) = self.current_message.take() {
            error!("Server returned us a message: {:?}", message);
            if let Some(pinky) = self.pinkies.pop_front() {
                pinky.swear(Confirmation::Nack(message));
            } else {
                self.messages.push_back(message);
            }
        }
    }

    fn drain(&mut self) -> Vec<BasicReturnMessage> {
        let mut messages = Vec::default();
        for promise in self
            .dropped_confirms
            .drain(..)
            .collect::<Vec<PinkySwear<Confirmation>>>()
        {
            if let Some(confirmation) = promise.try_wait() {
                if let Confirmation::Nack(message) = confirmation {
                    messages.push(message);
                }
            } else {
                self.dropped_confirms.push(promise);
            }
        }
        messages
    }
}
