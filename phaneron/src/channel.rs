/*
 * Phaneron media compositing software.
 * Copyright (C) 2023 SuperFlyTV AB
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use std::sync::{Arc, Mutex};

use std::fmt::Debug;

pub struct Channel<T>
where
    T: Clone,
{
    inner: Arc<Mutex<ChannelInner<T>>>,
}

impl<T> Channel<T>
where
    T: Clone,
{
    pub async fn subscribe(&self) -> tokio::sync::mpsc::Receiver<(T, ChannelSemaphore)> {
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        let mut inner = self.inner.lock().unwrap();
        inner.senders.push(sender);
        receiver
    }

    pub fn send(&self, semaphore_provider: &ChannelSemaphoreProvider, value: T) {
        let inner = self.inner.lock().unwrap();
        for sender in &inner.senders {
            let semaphore = semaphore_provider.get_semaphore();
            sender.blocking_send((value.clone(), semaphore)).ok();
        }
    }

    pub async fn no_receivers(&self) -> bool {
        self.inner.lock().unwrap().senders.is_empty()
    }
}

impl<T> Clone for Channel<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Default for Channel<T>
where
    T: Clone,
{
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ChannelInner::new())),
        }
    }
}

impl<T> Debug for Channel<T>
where
    T: Clone,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Channel").finish()
    }
}

struct ChannelInner<T>
where
    T: Clone,
{
    senders: Vec<tokio::sync::mpsc::Sender<(T, ChannelSemaphore)>>,
}

impl<T> ChannelInner<T>
where
    T: Clone,
{
    fn new() -> Self {
        ChannelInner { senders: vec![] }
    }
}

pub struct ChannelSemaphore {
    inner: tokio::sync::oneshot::Sender<()>,
}

impl ChannelSemaphore {
    pub fn new(semaphore: tokio::sync::oneshot::Sender<()>) -> Self {
        Self { inner: semaphore }
    }

    pub async fn signal(self) {
        self.inner.send(()).ok();
    }
}

#[derive(Debug, Default, Clone)]
pub struct ChannelSemaphoreProvider {
    inner: Arc<ChannelSemaphoreProviderInner>,
}

impl ChannelSemaphoreProvider {
    pub fn get_semaphore(&self) -> ChannelSemaphore {
        let mut semaphores = self.inner.semaphores.lock().unwrap();
        let (sender, receiver) = tokio::sync::oneshot::channel();
        semaphores.push(receiver);
        ChannelSemaphore::new(sender)
    }

    pub fn drain(&self) -> Vec<tokio::sync::oneshot::Receiver<()>> {
        let mut semaphores = self.inner.semaphores.lock().unwrap();
        semaphores.drain(..).collect()
    }
}

#[derive(Debug, Default)]
struct ChannelSemaphoreProviderInner {
    semaphores: std::sync::Mutex<Vec<tokio::sync::oneshot::Receiver<()>>>,
}
