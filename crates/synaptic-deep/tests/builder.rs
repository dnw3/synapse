#![cfg(feature = "config-builder")]

// Verify the builder module compiles and exports are accessible.
#[test]
fn builder_module_accessible() {
    // Verify the async function signature exists and is callable
    let _: fn(
        &synaptic_config::SynapticAgentConfig,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<
                        synaptic_graph::CompiledGraph<synaptic_graph::MessageState>,
                        synaptic_core::SynapticError,
                    >,
                > + '_,
        >,
    > = |config| Box::pin(synaptic_deep::build_agent_from_config(config));
}
