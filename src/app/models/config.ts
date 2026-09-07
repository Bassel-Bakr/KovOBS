// The config types come from the Rust structs, so the two cannot disagree.
// `cargo test` regenerates ./bindings and ./default-config.
export type { AppConfig as Config } from './bindings/app-config';
export type { Theme } from './bindings/theme';
export { DEFAULT_CONFIG } from './default-config';
