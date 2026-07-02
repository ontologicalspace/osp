// INV-C4 compile-fail: external crate SupersedeAuthority struct literal ile üretemez.
// `_private: ()` field private → capability bypass engelli.
// `AnchorGateContext { supersede_authority: Some(SupersedeAuthority { ... }) }` yazılamaz.
use osp_core::anchoring::gate::{SupersedeAuthority, SupersedeAuthorityLevel};

fn main() {
    // Bu satır derlenmemeli: _private field erişilebilir değil.
    let _auth = SupersedeAuthority {
        level: SupersedeAuthorityLevel::Operator,
        _private: (),
    };
}
