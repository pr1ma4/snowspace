using Microsoft.UI.Xaml;
using System.Runtime.InteropServices;

namespace SnowOverlay.Gui;

public partial class App : Application
{
    private Window? _window;

    public App()
    {
        UnhandledException += (_, e) =>
        {
            StartupDiagnostics.Write($"Unhandled WinUI exception: {e.Exception}");
            ShowError(e.Exception.ToString());
        };
        InitializeComponent();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        try
        {
            _window = new MainWindow();
            _window.Activate();
        }
        catch (Exception ex)
        {
            StartupDiagnostics.Write($"Launch failed: {ex}");
            ShowError(ex.ToString());
        }
    }

    private static void ShowError(string message) => MessageBoxW(IntPtr.Zero, message,
        "Snow Overlay — ошибка запуска", 0x10);

    [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "MessageBoxW")]
    private static extern int MessageBoxW(IntPtr hwnd, string text, string caption, uint type);
}

internal static class StartupDiagnostics
{
    private static readonly string LogPath = Path.Combine(AppContext.BaseDirectory, "startup.log");

    public static void Write(string message)
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(LogPath)!);
            File.AppendAllText(LogPath, $"{DateTime.Now:O} {message}{Environment.NewLine}");
        }
        catch { }
    }
}
