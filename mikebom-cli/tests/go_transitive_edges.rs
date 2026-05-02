// Milestone 055 — Integration tests for the Go transitive-edge resolver
// (FR-012). Hermetic against `proxy.golang.org` via `wiremock`.
//
// These tests are scaffolded under `#[ignore]` until the orchestration
// (T024) and `legacy::read()` integration (T025) land — at which point
// the bodies are filled in (T027 / T044) and the `#[ignore]` is removed.
// `#[ignore]` keeps the pre-PR gate green during incremental
// implementation: `cargo test` reports them as ignored rather than
// failed.

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "T027 — implement after T024+T025 wire resolver into legacy::read()"]
async fn ladder_step3_only_argo_fixture() {
    // T027: this test must:
    //   - start a wiremock::MockServer
    //   - register stubs for every <escaped-mod>/@v/<ver>.mod URL serving
    //     the synthesized files from tests/fixtures/go/argo-style-no-cache/proxy-mock/
    //   - set environment (PATH excluding go, GOMODCACHE empty tempdir,
    //     GOPROXY=mock URI, GOPRIVATE="")
    //   - invoke the Go ecosystem reader against
    //     tests/fixtures/go/argo-style-no-cache/argo-workflows/
    //   - assert ≥ 90% of go.sum modules have at least one outgoing edge
    //   - assert every emitted edge target is itself in go.sum (FR-003 / SC-006)
    //   - assert FR-009 summary line was emitted with proxy:N>0
    unimplemented!("see #[ignore] reason");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "T044 — implement after T024+T025 wire resolver"]
async fn offline_makes_no_network_calls() {
    // T044: SC-005 verification.
    //   - start a wiremock::MockServer with a catch-all 500 stub
    //   - point WorkspaceContext.goproxy at the mock URL
    //   - run GraphResolver::resolve(&ctx) with ctx.offline = true
    //     against argo-style-no-cache/argo-workflows/
    //   - assert mock_server.received_requests().await.unwrap().len() == 0
    unimplemented!("see #[ignore] reason");
}
