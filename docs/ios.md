# Horizon on iPhone and iPad

The game builds for iOS and the iOS simulator, draws with Metal and plays with touch controls.
There is no App Store build: the app carries resources prepared from your own copy of Minecraft,
so you build it yourself and install it on your own devices.

## Build and install

You need a Mac with Xcode 27 or later, the Rust targets
(`rustup target add aarch64-apple-ios aarch64-apple-ios-sim`) and prepared resources:
[Build from source](../README.md#build-from-source), then `horizon-prepare sounds` for sound.

```sh
# The simulator: signed ad hoc.
packaging/ios/package.sh simulator .local/horizon-client/26.3 .local/ios
xcrun simctl install booted .local/ios/simulator/Horizon.app
xcrun simctl launch booted dev.horizon.ios

# A device (not yet tried: written from the signing steps for any iOS app): an Apple
# Development certificate and a provisioning profile that covers the bundle id and the device
# (Xcode makes one when it signs any app for that device).
IOS_SIGN_IDENTITY="Apple Development: …" IOS_PROFILE=~/path/to/profile.mobileprovision \
IOS_BUNDLE_ID=com.example.horizon packaging/ios/package.sh device .local/horizon-client/26.3 .local/ios
xcrun devicectl device install app --device UDID .local/ios/device/Horizon.app
```

`package.sh` builds `horizon-client` with the `fast` profile and without the `controllers` feature
(gilrs and hidapi have no iOS backend), and puts the resources in the app as `horizon-resources`.
The game folder (saves, screenshots, `options.txt`, logs) is `Documents/horizon` in the app's
container, which the Files app shows under On My iPhone. Arguments after the bundle id in
`simctl launch` reach the game: `--control --control-socket ctl.sock` puts the control socket in
that folder. Its full path is longer than a Unix socket path may be, so on the Mac `cd` into the
folder (`xcrun simctl get_app_container booted BUNDLE_ID data`, then `Documents/horizon`) and run
`horizon-ctl --socket ctl.sock`. The world is saved when the app
goes to the background, where iOS may end it without warning.

## Touch controls

Pocket Edition's controls, drawn in its pixel style. They are on in the iOS build only, and drawn
once a finger first touches the screen.

| Where | What it does |
| --- | --- |
| Title screen and menus | A finger is the mouse pointer: tap a button, drag a slider |
| Back (top left, in a world's menus) | Escape: closes the inventory and other screens, resumes from pause |
| D-pad (bottom left) | Forward, back and the sides; the forward diagonals show while forward is held, and a thumb slides between arrows. A double tap on forward sprints. The middle toggles sneaking, and goes down while flying |
| Jump (bottom right) | Jumps; a double tap flies in creative, and holding it goes up while flying |
| Anywhere else | Drag to turn the view. Tap to attack the mob under the crosshair, or else use the held item (place a block, open a door or chest). Hold still: a ring fills, then the finger mines until it lifts, or eats, drinks, draws a bow or blocks when the held item is used over time |
| Hotbar | Tap or slide to select a slot |
| ••• beside the hotbar | The inventory |
| II (top right) | Pause |

Options > Horizon Settings > Controller Settings > **Touch Movement** swaps the D-pad for a
joystick that appears where the left thumb lands and sprints pushed all the way forward. Controls'
Sneak: Toggle and Sprint: Toggle work as with a keyboard.

The system's edge swipes (home, Control Center) are asked to wait for a second swipe while the
game is open, so the controls along the edges keep it (not yet tried on a device).
`horizon-ctl touch start|move|end|cancel ID X Y` sends a finger as the screen would, on every
platform, so the controls can be tested on a Mac.

## What is different on iOS

- Windows come from a vendored winit 0.30 (`third_party/winit`) that launches on the UIScene life
  cycle, which apps built with the iOS 27 SDK need; the root `Cargo.toml` says what changed.
- Text copied in the game stays in the game: Horizon does not reach the system clipboard there.
- The iOS simulator's Metal has no indirect draws, so the world draws one section at a time there;
  iPhones and iPads have them.
- Not wired up yet: typing (chat, signs, world names: there is no on-screen keyboard for touch),
  scrolling lists by dragging them (their scroll bars work), game controllers, and code mods (the
  `mods` feature is not built for iOS). Multiplayer, LAN and Play with Friends have not been tried
  on iOS, and neither has a physical device.
