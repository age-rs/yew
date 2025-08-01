use super::{Key, VNode};
use crate::html::ImplicitClone;

/// This struct represents a suspendable DOM fragment.
#[derive(Clone, Debug, PartialEq)]
pub struct VSuspense {
    /// Child nodes.
    pub(crate) children: VNode,
    /// Fallback nodes when suspended.
    pub(crate) fallback: VNode,
    /// Whether the current status is suspended.
    pub(crate) suspended: bool,
    /// The Key.
    pub(crate) key: Option<Key>,
}

impl ImplicitClone for VSuspense {}

impl VSuspense {
    pub fn new(children: VNode, fallback: VNode, suspended: bool, key: Option<Key>) -> Self {
        Self {
            children,
            fallback,
            suspended,
            key,
        }
    }
}

#[cfg(feature = "ssr")]
mod feat_ssr {
    use super::*;
    use crate::feat_ssr::VTagKind;
    use crate::html::AnyScope;
    use crate::platform::fmt::BufWriter;
    use crate::virtual_dom::Collectable;

    impl VSuspense {
        pub(crate) async fn render_into_stream(
            &self,
            w: &mut BufWriter,
            parent_scope: &AnyScope,
            hydratable: bool,
            parent_vtag_kind: VTagKind,
        ) {
            let collectable = Collectable::Suspense;

            if hydratable {
                collectable.write_open_tag(w);
            }

            // always render children on the server side.
            self.children
                .render_into_stream(w, parent_scope, hydratable, parent_vtag_kind)
                .await;

            if hydratable {
                collectable.write_close_tag(w);
            }
        }
    }
}

#[cfg(any(not(target_arch = "wasm32"), target_os = "wasi"))]
#[cfg(feature = "ssr")]
#[cfg(test)]
mod ssr_tests {
    use std::rc::Rc;
    use std::time::Duration;

    use tokio::task::{spawn_local, LocalSet};
    use tokio::test;

    use crate::platform::time::sleep;
    use crate::prelude::*;
    use crate::suspense::{Suspension, SuspensionResult};
    use crate::ServerRenderer;

    #[cfg(not(target_os = "wasi"))]
    #[test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_suspense() {
        #[derive(PartialEq)]
        pub struct SleepState {
            s: Suspension,
        }

        impl SleepState {
            fn new() -> Self {
                let (s, handle) = Suspension::new();

                // we use tokio spawn local here.
                spawn_local(async move {
                    // we use tokio sleep here.
                    sleep(Duration::from_millis(50)).await;

                    handle.resume();
                });

                Self { s }
            }
        }

        impl Reducible for SleepState {
            type Action = ();

            fn reduce(self: Rc<Self>, _action: Self::Action) -> Rc<Self> {
                Self::new().into()
            }
        }

        #[hook]
        pub fn use_sleep() -> SuspensionResult<Rc<dyn Fn()>> {
            let sleep_state = use_reducer(SleepState::new);

            if sleep_state.s.resumed() {
                Ok(Rc::new(move || sleep_state.dispatch(())))
            } else {
                Err(sleep_state.s.clone())
            }
        }

        #[derive(PartialEq, Properties, Debug)]
        struct ChildProps {
            name: String,
        }

        #[component]
        fn Child(props: &ChildProps) -> HtmlResult {
            use_sleep()?;
            Ok(html! { <div>{"Hello, "}{&props.name}{"!"}</div> })
        }

        #[component]
        fn Comp() -> Html {
            let fallback = html! {"loading..."};

            html! {
                <Suspense {fallback}>
                    <Child name="Jane" />
                    <Child name="John" />
                    <Child name="Josh" />
                </Suspense>
            }
        }

        let local = LocalSet::new();

        let s = local
            .run_until(async move {
                ServerRenderer::<Comp>::new()
                    .hydratable(false)
                    .render()
                    .await
            })
            .await;

        assert_eq!(
            s,
            "<div>Hello, Jane!</div><div>Hello, John!</div><div>Hello, Josh!</div>"
        );
    }
}
