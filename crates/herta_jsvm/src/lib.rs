//! Isolated, asynchronous JavaScript extension execution.

mod cron;
mod engine;
mod host_queue;
mod runtime;
mod source;
pub use cron::CronScheduler;
pub use runtime::JsRuntime;
pub use source::Snapshot;

#[cfg(test)]
mod tests {
    use rquickjs::{AsyncContext, AsyncRuntime};

    #[tokio::test]
    async fn quickjs_supports_async_contexts_on_this_target() {
        let runtime = AsyncRuntime::new().unwrap();
        runtime.set_memory_limit(16 * 1024 * 1024).await;
        runtime.set_max_stack_size(512 * 1024).await;
        let context = AsyncContext::full(&runtime).await.unwrap();
        let answer: i32 = context.with(|ctx| ctx.eval("21 * 2")).await.unwrap();
        assert_eq!(answer, 42);
        drop(context);
        drop(runtime);
    }
}
