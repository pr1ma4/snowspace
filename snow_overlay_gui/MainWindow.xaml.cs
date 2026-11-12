using System.Diagnostics;
using System.Globalization;
using System.Drawing;
using System.Runtime.InteropServices;
using Forms = System.Windows.Forms;
using Microsoft.UI.Xaml;
using Microsoft.UI.Windowing;
using Windows.Graphics;

namespace SnowOverlay.Gui;

public sealed partial class MainWindow : Window
{
    private readonly Process? _engine;
    private Forms.NotifyIcon? _trayIcon;
    private Forms.ContextMenuStrip? _trayMenu;
    private Forms.ToolStripMenuItem? _traySnowItem;
    private Icon? _trayIconImage;
    private Icon? _windowIcon;
    private bool _closing;
    private bool _exitRequested;

    public MainWindow()
    {
        InitializeComponent();
        var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
        var windowId = Microsoft.UI.Win32Interop.GetWindowIdFromWindow(hwnd);
        AppWindow.GetFromWindowId(windowId).Resize(new SizeInt32(480, 600));
        var iconPath = Path.Combine(AppContext.BaseDirectory, "Assets", "SnowOverlay.ico");
        if (File.Exists(iconPath))
        {
            _windowIcon = new Icon(iconPath);
            SendMessage(hwnd, WmSetIcon, new IntPtr(1), _windowIcon.Handle);
            SendMessage(hwnd, WmSetIcon, IntPtr.Zero, _windowIcon.Handle);
        }
        AppWindow.Closing += AppWindow_Closing;
        SnowToggle.Toggled += SnowToggle_Toggled;
        RateSlider.ValueChanged += RateSlider_ValueChanged;
        TractorButton.Click += TractorButton_Click;
        Closed += MainWindow_Closed;
        UpdateRateLabel();
        CreateTrayIcon();

        var enginePath = Path.Combine(AppContext.BaseDirectory, "snow_overlay.exe");
        if (!File.Exists(enginePath))
        {
            StatusText.Text = "Не найден snow_overlay.exe. Пересобери проект через Cargo.";
            SnowToggle.IsEnabled = false;
            RateSlider.IsEnabled = false;
            TractorButton.IsEnabled = false;
            return;
        }

        try
        {
            var start = new ProcessStartInfo(enginePath,
                $"--worker --rate 12 --exclude-pid {Environment.ProcessId}")
            {
                UseShellExecute = false,
                CreateNoWindow = true,
                RedirectStandardInput = true,
                WorkingDirectory = AppContext.BaseDirectory
            };
            _engine = Process.Start(start);
            if (_engine is null) throw new InvalidOperationException("The snow engine did not start.");
            Send("enabled 1");
            SendRate();
            StatusText.Text = "Снег идёт поверх окон. Закрытие окна скрывает приложение в трей.";
        }
        catch (Exception ex)
        {
            StartupDiagnostics.Write($"Rust engine startup failed: {ex}");
            StatusText.Text = $"Не удалось запустить снег: {ex.Message}";
            SnowToggle.IsEnabled = false;
            RateSlider.IsEnabled = false;
            TractorButton.IsEnabled = false;
        }
    }

    private void SnowToggle_Toggled(object sender, RoutedEventArgs e)
    {
        Send($"enabled {(SnowToggle.IsOn ? 1 : 0)}");
        SnowStateText.Text = SnowToggle.IsOn ? "Идёт снег" : "Снегопад остановлен";
        UpdateTraySnowItem();
    }

    private void RateSlider_ValueChanged(object sender, Microsoft.UI.Xaml.Controls.Primitives.RangeBaseValueChangedEventArgs e)
    {
        UpdateRateLabel();
        SendRate();
    }

    private void TractorButton_Click(object sender, RoutedEventArgs e) => Send("tractor");

    private void CreateTrayIcon()
    {
        _trayMenu = new Forms.ContextMenuStrip();
        _traySnowItem = new Forms.ToolStripMenuItem();
        _traySnowItem.Click += (_, _) => SnowToggle.IsOn = !SnowToggle.IsOn;
        var openItem = new Forms.ToolStripMenuItem("Открыть приложение");
        openItem.Click += (_, _) => ShowWindow();
        var exitItem = new Forms.ToolStripMenuItem("Выход");
        exitItem.Click += (_, _) => ExitFromTray();
        _trayMenu.Items.Add(_traySnowItem);
        _trayMenu.Items.Add(new Forms.ToolStripSeparator());
        _trayMenu.Items.Add(openItem);
        _trayMenu.Items.Add(exitItem);
        _trayMenu.Opening += (_, _) => UpdateTraySnowItem();

        var iconPath = Path.Combine(AppContext.BaseDirectory, "Assets", "SnowOverlay.ico");
        _trayIconImage = File.Exists(iconPath) ? new Icon(iconPath) : (Icon)SystemIcons.Application.Clone();
        _trayIcon = new Forms.NotifyIcon
        {
            Icon = _trayIconImage,
            Text = "Снег на рабочем столе",
            ContextMenuStrip = _trayMenu,
            Visible = true
        };
        _trayIcon.MouseClick += (_, e) =>
        {
            if (e.Button == Forms.MouseButtons.Left) ShowWindow();
        };
        UpdateTraySnowItem();
    }

    private void UpdateTraySnowItem()
    {
        if (_traySnowItem is null) return;
        _traySnowItem.Checked = SnowToggle.IsOn;
        _traySnowItem.Text = SnowToggle.IsOn ? "Выключить снег" : "Включить снег";
    }

    private void ShowWindow()
    {
        AppWindow.Show(true);
        Activate();
    }

    private void AppWindow_Closing(AppWindow sender, AppWindowClosingEventArgs args)
    {
        if (_exitRequested) return;
        args.Cancel = true;
        AppWindow.Hide();
    }

    private void ExitFromTray()
    {
        _exitRequested = true;
        _trayIcon?.Dispose();
        _trayIcon = null;
        Close();
    }

    private const uint WmSetIcon = 0x0080;

    [DllImport("user32.dll", EntryPoint = "SendMessageW")]
    private static extern IntPtr SendMessage(IntPtr hwnd, uint message, IntPtr wParam, IntPtr lParam);

    private void UpdateRateLabel() => RateText.Text = $"{RateSlider.Value:0} снежинок/сек";

    private void SendRate() => Send($"rate {RateSlider.Value.ToString("0.0", CultureInfo.InvariantCulture)}");

    private void Send(string command)
    {
        if (_closing || _engine is null) return;
        try
        {
            if (!_engine.HasExited) _engine.StandardInput.WriteLine(command);
        }
        catch (InvalidOperationException) { }
        catch (IOException) { }
    }

    private void MainWindow_Closed(object sender, WindowEventArgs args)
    {
        _closing = true;
        _trayIcon?.Dispose();
        _trayMenu?.Dispose();
        _trayIconImage?.Dispose();
        _windowIcon?.Dispose();
        _trayIcon = null;
        _trayMenu = null;
        _trayIconImage = null;
        try
        {
            if (_engine is not null && !_engine.HasExited)
            {
                _engine.StandardInput.WriteLine("quit");
                if (!_engine.WaitForExit(1500)) _engine.Kill(true);
            }
        }
        catch (InvalidOperationException) { }
        catch (IOException) { }
        catch (System.ComponentModel.Win32Exception) { }
        _engine?.Dispose();
    }
}
