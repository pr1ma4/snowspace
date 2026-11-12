# Snow Overlay

A small Windows desktop overlay written in Rust. It draws transparent falling snow over other windows and gradually builds a snowbank along the bottom of the virtual desktop. The overlay is click-through and does not appear in the taskbar.

## Run

At startup the app asks for a snow rate from 1 to 60 flakes per second. Press Enter to use the default value, 12. The console closes after the value is entered while the overlay keeps running.

You can also pass the rate directly:

```powershell
cargo run --release -- 24
```

## Build

Install the Rust MSVC toolchain, then run from this folder:

```powershell
cargo run --release
```

To build a standalone executable:

```powershell
cargo build --release
```

The executable is created at `target\release\snow_overlay.exe`. When launching the executable directly, enter the rate in the startup console, or pass it as an argument, for example `snow_overlay.exe 24`.

## WinUI control panel

The WinUI 3 control panel and managed Rust overlay engine are in `snow_overlay_gui`. Build and launch the panel from the project root with:

```powershell
dotnet run --project .\snow_overlay_gui\SnowOverlay.Gui.csproj -c Release
```

The build compiles the Rust engine and copies it beside the WinUI app. The panel controls snowfall, applies frequency changes immediately, and has a button to call the tractor. The tractor also clears the snow automatically when the average snowbank reaches about 28 px. Snow accumulates along the top edges of visible, non-maximized windows; moving or minimizing a window makes its snow fall onto windows below or the bottom of the desktop.

The snowflake icon stays in the notification area. Left-click it to open the panel; right-click it to toggle snowfall, reopen the panel, or exit. Closing the panel window hides it to the tray and leaves the overlay running.
