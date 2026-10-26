# Horizon on iPhone and iPad

The game builds for iOS and the iOS simulator, draws with Metal, plays with Pocket Edition's
touch controls and runs Horizon mods.

## Build and install

There is no App Store build: the app carries game files made from your own copy of Minecraft, so
you build it on a Mac and install it on your own devices. Keep the app and its `.local/` folder to
yourself.

### What you need

- A Mac with **Xcode 27** or later. Open it once to accept its license and install the iOS
  platform (Xcode > Settings > Components).
- **Rust** through [rustup](https://rustup.rs), 1.90 or newer, with the iOS targets:
  `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`.
- A **Java 25 JDK**, to prepare the game files once: `brew install openjdk@25`, then
  `export JAVA_HOME="$(brew --prefix openjdk@25)" PATH="$(brew --prefix openjdk@25)/bin:$PATH"`.
- **Minecraft: Java Edition 26.3**, installed with the official launcher and played once, and its
  server jar: the `downloads.server.url` in the launcher's `versions/26.3/26.3.json`, or
  [minecraft.net/download/server](https://www.minecraft.net/en-us/download/server) while 26.3 is the
  current release.

### 1. Prepare the game files (once)

```sh
git clone https://github.com/woke/horizon-duo.git
cd horizon-duo
cargo run --release -p horizon-prepare -- --prepare --server-jar ~/Downloads/server.jar
cargo run --release -p horizon-prepare -- sounds   # optional: the game is silent without it
```

They land in `.local/horizon-client/26.3`, about 700 MB with the sounds. If your launcher keeps
the game somewhere other than `~/Library/Application Support/minecraft`, add `--minecraft-dir PATH`.

### 2. Play in the simulator

```sh
packaging/ios/package.sh simulator .local/horizon-client/26.3 .local/ios
xcrun simctl list devices available          # pick an iPhone or iPad
xcrun simctl boot "iPhone 17 Pro"            # skip if one is already booted
open -a Simulator
xcrun simctl install booted .local/ios/simulator/Horizon.app
xcrun simctl launch booted dev.horizon.ios
```

The first build takes a few minutes; later ones rebuild what changed. In the Simulator a click is
a touch, so the touch controls work with the mouse.

### 3. Play on your iPhone or iPad

Not yet tried on a physical device. There are two ways in.

**Sideload the .ipa** (no Xcode account needed on the Mac):

```sh
packaging/ios/package.sh device .local/horizon-client/26.3 .local/ios
```

Without `IOS_PROFILE` this makes `.local/ios/device/Horizon.ipa`, signed ad hoc. Install it with a
sideloading tool such as [Sideloadly](https://sideloadly.io) or [AltStore](https://altstore.io),
which signs it again with your Apple ID. With a free Apple ID the app stops opening after 7 days
until you install it again; if the tool says the bundle id is taken, let it change it. The .ipa
holds files made from your Minecraft: don't share or upload it.

**Or sign it yourself** with Xcode's signing:

1. Sign in to Xcode with your Apple account (Xcode > Settings > Accounts). A free account works,
   but its apps stop opening after 7 days until you install them again.
2. On the device, turn on Developer Mode (Settings > Privacy & Security > Developer Mode), and
   connect it to the Mac once by cable.
3. Get a provisioning profile for your device and a bundle id of your own: in Xcode, make a new iOS
   App project with that bundle id (say `com.yourname.horizon`), pick your team under Signing &
   Capabilities, and run it on the device once. Xcode registers the device and saves the profile in
   `~/Library/Developer/Xcode/UserData/Provisioning Profiles`.
4. Build, sign and install Horizon with that profile:

   ```sh
   security find-identity -v -p codesigning   # your "Apple Development: …" identity
   xcrun devicectl list devices               # your device's identifier
   IOS_SIGN_IDENTITY="Apple Development: Your Name (ABCDE12345)" \
   IOS_PROFILE="$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles/<profile>.mobileprovision" \
   IOS_BUNDLE_ID=com.yourname.horizon \
   packaging/ios/package.sh device .local/horizon-client/26.3 .local/ios
   xcrun devicectl device install app --device <identifier> .local/ios/device/Horizon.app
   ```

5. With a free account, the first launch asks you to trust the developer: Settings > General >
   VPN & Device Management.

### Mods

Mods are the same `.hmod` files as on desktops ([mods.md](mods.md)). In the Files app, put them in
On My iPhone > Horizon > horizon > `horizon-mods`, restart Horizon, and turn them on in Options >
Mods. iOS lets no app run code it generates, so code mods run on wasmtime's Pulley interpreter:
they work, more slowly than on a desktop.

### Troubleshooting

- **`can't find crate for core` (or `std`)**: `cargo` isn't rustup's (Homebrew's `rust` has no iOS
  targets). Put rustup's first: `export PATH="$(dirname "$(rustup which cargo)"):$PATH"`.
- **`Unable to locate a Java Runtime`** from `horizon-prepare`: `JAVA_HOME` isn't a Java 25 JDK
  (see What you need).
- **The game stops at "Prepare resources"**: step 1 didn't finish, or `package.sh` was given
  another folder than `.local/horizon-client/26.3`.

### How the app is put together

`package.sh` builds `horizon-client` with the `fast` profile and the `mods` feature, without
`controllers` (gilrs and hidapi have no iOS backend), and puts the resources in the app as
`horizon-resources`.
The game folder (saves, screenshots, `options.txt`, logs) is `Documents/horizon` in the app's
container, which the Files app shows under On My iPhone. Arguments after the bundle id in
`simctl launch` reach the game: `--control --control-socket ctl.sock` puts the control socket in
that folder. Its full path is longer than a Unix socket path may be, so on the Mac `cd` into the
folder (`xcrun simctl get_app_container booted BUNDLE_ID data`, then `Documents/horizon`) and run
`horizon-ctl --socket ctl.sock`. The world is saved when the app goes to the background, where
iOS may end it without warning.

## Touch controls

Pocket Edition's controls, drawn in its pixel style. They are on in the iOS build only, and drawn
once a finger first touches the screen.

| Where | What it does |
| --- | --- |
| Title screen and menus | A finger is the mouse. A tap clicks where the finger lifts. A drag presses where the finger landed and follows it, so sliders drag. A touch the system takes clicks nothing |
| Lists | Drag a list to scroll it: worlds, servers, options, chat, a villager's trades, the stonecutter, the loom and creative's items. A scrollbar still drags, and a slider in a list moves when you drag it sideways |
| Inventories: picking up and placing | Tap a stack to pick it up, and tap a slot to put it down. The stack you hold stays where your finger lifted. Double tap a stack to gather all of its kind |
| Inventories: dragging | Drag a stack from its slot to another to move it. On a different item they swap. Lifted on a gap, it goes back. Holding a stack, drag across slots to spread it evenly. Drag a stack out of the panel to throw it; a tap out there throws nothing |
| Inventories: long press | A ring fills under the finger for half a second, then the long press is the mouse's right button. On a stack, it takes half. With a stack in hand, it places one; drag on to place one on each slot you cross. Outside the panel, it drops one. On the crafting result, it crafts once. On a creative item, it gives a full stack. On a recipe with alternatives, it shows them |
| Shift (bottom left, in inventories) | The keyboard's Shift, as on the iPhone keyboard. Tap it to shift the next tap, double tap it to lock it (underlined), tap again to unlock, or hold it while you tap. Shift and a tap moves a stack across (into a chest and back), and on the crafting result crafts all it can. Creative's trash empties the inventory only while a finger holds Shift |
| Tooltips | Rest a finger on a stack or a button: its tooltip shows above the finger, out from under it |
| Back (top left, in a world's menus) | Escape: closes the inventory and other screens, resumes from pause |
| D-pad (bottom left) | Forward, back and the sides; the forward diagonals show while forward is held, and a thumb slides between arrows. A double tap on forward sprints. The middle toggles sneaking, and goes down while flying |
| Jump (bottom right) | Jumps; a double tap flies in creative, and holding it goes up while flying |
| Anywhere else | Drag to turn the view. Tap to attack the mob under the crosshair, or else use the held item (place a block, open a door or chest). Hold still: a ring fills, then the finger mines until it lifts, or eats, drinks, draws a bow or blocks when the held item is used over time |
| Hotbar | Tap or slide to select a slot |
| ••• beside the hotbar | The inventory |
| II (top right) | Pause |
| Speech bubble (left of II) | Opens chat |

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
- Code mods run on wasmtime's Pulley interpreter instead of cranelift's native code.
- Not wired up yet: typing (chat, signs, world names: there is no on-screen keyboard for touch),
  scrolling lists by dragging them (their scroll bars work) and game controllers. Multiplayer, LAN
  and Play with Friends have not been tried on iOS, and neither has a physical device.
