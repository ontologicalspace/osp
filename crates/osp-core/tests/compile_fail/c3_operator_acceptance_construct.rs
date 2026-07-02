// INV-C3 compile-fail: external crate OperatorAcceptance struct literal ile üretemez.
// `_private: ()` field private → struct literal compile error.
use osp_core::anchoring::store::OperatorAcceptance;

fn main() {
    // Bu satır derlenmemeli: _private field erişilebilir değil.
    let _cap = OperatorAcceptance { _private: () };
}
