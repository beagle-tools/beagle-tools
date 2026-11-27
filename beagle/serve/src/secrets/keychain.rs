//! The macOS Keychain as a [`SecretStore`]: one generic-password item per
//! name (account = the name, service = [`SERVICE`]), through the Security
//! framework, so a value never appears on a command line (`security
//! add-generic-password -w` would show it in `ps`).
//!
//! Off macOS there is no Keychain: reads find nothing (push falls back to
//! the environment and `[push]`), writes fail with what to do instead.
//!
//! [`Keychain::non_interactive`] never shows a dialog: a read that would
//! ask the user ("beagle wants to use your confidential information…",
//! typically after an update changed the binary) fails at once with
//! [`NEEDS_APPROVAL`] instead of waiting for a click. `beagle doctor`,
//! `beagle serve status` and `beagle server-mode` read it this way.

use super::SecretStore;

/// The Keychain service the items are stored under.
pub const SERVICE: &str = "dev.beagletools.serve";

/// What a non-interactive read says when the Keychain wants a click.
pub const NEEDS_APPROVAL: &str = "Keychain access needs approval — run `beagle serve secrets sync`";

/// The Keychain under one service name.
#[derive(Debug, Clone)]
pub struct Keychain {
    service: String,
    /// Whether reads may show a Keychain dialog.
    interactive: bool,
}

impl Default for Keychain {
    fn default() -> Self {
        Self::new(SERVICE)
    }
}

impl Keychain {
    /// The items under `service`.
    #[must_use]
    pub fn new(service: &str) -> Self {
        Self {
            service: service.to_owned(),
            interactive: true,
        }
    }

    /// The default service, read without ever showing a dialog.
    #[must_use]
    pub fn non_interactive() -> Self {
        Self {
            interactive: false,
            ..Self::default()
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use security_framework::os::macos::keychain::SecKeychain;
    use security_framework::passwords;

    /// `errSecItemNotFound`.
    const NOT_FOUND: i32 = -25300;
    /// `errSecInteractionNotAllowed`: the read would have shown a dialog.
    const INTERACTION_NOT_ALLOWED: i32 = -25308;

    #[allow(clippy::needless_pass_by_value)]
    fn describe(e: security_framework::base::Error) -> String {
        format!("Keychain: {e}")
    }

    pub fn get(service: &str, name: &str, interactive: bool) -> Result<Option<String>, String> {
        // Held for the read: Keychain calls fail instead of prompting. It
        // is process-wide, and dropping it allows dialogs again.
        let _quiet = if interactive {
            None
        } else {
            Some(SecKeychain::disable_user_interaction().map_err(describe)?)
        };
        match passwords::get_generic_password(service, name) {
            Ok(bytes) => String::from_utf8(bytes)
                .map(Some)
                .map_err(|_| format!("Keychain item {name} is not UTF-8")),
            Err(e) if e.code() == NOT_FOUND => Ok(None),
            Err(e) if e.code() == INTERACTION_NOT_ALLOWED => Err(super::NEEDS_APPROVAL.to_owned()),
            Err(e) => Err(describe(e)),
        }
    }

    pub fn set(service: &str, name: &str, value: &str) -> Result<(), String> {
        passwords::set_generic_password(service, name, value.as_bytes()).map_err(describe)
    }

    pub fn delete(service: &str, name: &str) -> Result<bool, String> {
        match passwords::delete_generic_password(service, name) {
            Ok(()) => Ok(true),
            Err(e) if e.code() == NOT_FOUND => Ok(false),
            Err(e) => Err(describe(e)),
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    const OFF_MACOS: &str =
        "the Keychain is macOS-only; set BEAGLE_APNS_* in the server's environment instead";

    // Same signature as the macOS one.
    #[allow(clippy::unnecessary_wraps)]
    pub fn get(_: &str, _: &str, _: bool) -> Result<Option<String>, String> {
        Ok(None)
    }

    pub fn set(_: &str, _: &str, _: &str) -> Result<(), String> {
        Err(OFF_MACOS.to_owned())
    }

    pub fn delete(_: &str, _: &str) -> Result<bool, String> {
        Err(OFF_MACOS.to_owned())
    }
}

impl SecretStore for Keychain {
    fn get(&self, name: &str) -> Result<Option<String>, String> {
        imp::get(&self.service, name, self.interactive)
    }

    fn set(&self, name: &str, value: &str) -> Result<(), String> {
        imp::set(&self.service, name, value)
    }

    fn delete(&self, name: &str) -> Result<bool, String> {
        imp::delete(&self.service, name)
    }
}
