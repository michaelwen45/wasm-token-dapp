## WASM Experiments - WIP
Experimenting with uploading files to arweave and creating nfts with wasm. The motivation for this was being able to use the arloader Rust code to create and upload arweave transactions and to be able to interact with with Solana using the Rust solana sdk in a front end application. Working toward toy example of creating a turn key token from uploading a file and approving a Solana transaction with the Phantom wallet.

### Features
* No javascript
* Merklize file bytes
* Connect to Phantom wallet
* [Sycamore](https://github.com/sycamore-rs/sycamore) reactive front end with Redux style [store](src/store.rs) using Sycamore context
* [tailwindcss](https://tailwindcss.com/docs/installation) styles - full tree shaking exclude unused styles

### Usage
```
npm install
cargo install --locked trunk
cargo trunk serve --release
```
See output in console.

### Parking Lot
* [wasm-bindgen-rayon](https://docs.rs/crate/wasm-bindgen-rayon/latest)