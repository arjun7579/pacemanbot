use std::{sync::Arc, time::Duration};

use serenity::{client::Context, model::prelude::Ready};
use tokio::time::sleep;

use crate::{
    cache::CacheManager,
    dispatcher::Dispatcher,
    eprintln,
    ws::{consts::WS_TIMEOUT_FOR_RETRY, WSManager},
};

use super::ArcMutex;

pub async fn ws_event_loop(
    ctx: Arc<Context>,
    cache_manager: ArcMutex<CacheManager>,
    ws_manager: ArcMutex<WSManager>,
) {
    loop {
        loop {
            let mut locked_ws_mgr = ws_manager.lock().await;
            let response = match locked_ws_mgr.get_next().await {
                Some(response) => response,
                None => break,
            };
            let dispatcher = Dispatcher {
                ctx: ctx.clone(),
                response,
                cache_manager: cache_manager.clone(),
            };
            match dispatcher.dispatch().await {
                Ok(_) => (),
                Err(err) => {
                    eprintln!("Dispatch error: {}", err);
                    continue;
                }
            };
        }
        let mut locked_ws_mgr = ws_manager.lock().await;
        *locked_ws_mgr = match WSManager::new().await {
            Ok(mgr) => mgr,
            Err(err) => {
                eprintln!("WSManager init error: {}", err);
                println!("Trying again in {} seconds...", WS_TIMEOUT_FOR_RETRY);
                sleep(Duration::from_secs(WS_TIMEOUT_FOR_RETRY)).await;
                continue;
            }
        }
    }
}

pub async fn handle_ready(
    ctx: Context,
    ready: Ready,
    cache_manager: ArcMutex<CacheManager>,
    ws_manager: ArcMutex<WSManager>,
) {
    println!("{} is connected!", ready.user.name);
    let cache_manager = cache_manager.clone();
    let ctx = Arc::new(ctx);
    tokio::spawn(async move { ws_event_loop(ctx, cache_manager, ws_manager.clone()).await });
}
