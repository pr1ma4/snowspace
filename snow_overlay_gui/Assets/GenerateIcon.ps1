Add-Type -AssemblyName System.Drawing
$drawingAssembly = [System.Drawing.Bitmap].Assembly.Location
Add-Type -ReferencedAssemblies @($drawingAssembly) -TypeDefinition @'
using System;
using System.IO;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Collections.Generic;

public static class SnowIconGenerator
{
    static Bitmap Render(int size)
    {
        var bmp = new Bitmap(size, size, PixelFormat.Format32bppArgb);
        using (var g = Graphics.FromImage(bmp))
        {
            g.SmoothingMode = SmoothingMode.AntiAlias;
            g.InterpolationMode = InterpolationMode.HighQualityBicubic;
            g.PixelOffsetMode = PixelOffsetMode.HighQuality;
            float s = size / 256f;
            g.ScaleTransform(s, s);

            using (var path = new GraphicsPath())
            {
                path.AddArc(8, 8, 116, 116, 180, 90);
                path.AddArc(132, 8, 116, 116, 270, 90);
                path.AddArc(132, 132, 116, 116, 0, 90);
                path.AddArc(8, 132, 116, 116, 90, 90);
                path.CloseFigure();
                using (var bg = new LinearGradientBrush(new Point(20, 12), new Point(220, 248),
                    Color.FromArgb(255, 47, 69, 91), Color.FromArgb(255, 25, 37, 52)))
                    g.FillPath(bg, path);
            }

            // Quiet falling snow, with one distinct six-armed flake.
            using (var flakes = new SolidBrush(Color.FromArgb(225, 239, 247, 255)))
            {
                g.FillEllipse(flakes, 59, 57, 9, 9);
                g.FillEllipse(flakes, 188, 52, 7, 7);
                g.FillEllipse(flakes, 202, 111, 8, 8);
                g.FillEllipse(flakes, 83, 126, 6, 6);
                g.FillEllipse(flakes, 142, 91, 6, 6);
            }
            using (var pen = new Pen(Color.FromArgb(248, 250, 255), 7) { StartCap = LineCap.Round, EndCap = LineCap.Round })
            {
                PointF c = new PointF(128, 105);
                for (int arm = 0; arm < 3; arm++)
                {
                    double a = arm * Math.PI / 3;
                    float dx = (float)(Math.Cos(a) * 27), dy = (float)(Math.Sin(a) * 27);
                    g.DrawLine(pen, c.X - dx, c.Y - dy, c.X + dx, c.Y + dy);
                    foreach (float dir in new float[] { -1, 1 })
                    {
                        float ex = c.X + dx * dir, ey = c.Y + dy * dir;
                        float bx = ex - dx * dir * .3f, by = ey - dy * dir * .3f;
                        double branch = a + dir * Math.PI / 4;
                        g.DrawLine(pen, ex, ey, bx + (float)Math.Cos(branch) * 12, by + (float)Math.Sin(branch) * 12);
                        branch = a - dir * Math.PI / 4;
                        g.DrawLine(pen, ex, ey, bx + (float)Math.Cos(branch) * 12, by + (float)Math.Sin(branch) * 12);
                    }
                }
            }

            // Soft snowbank sweeping across the lower edge.
            using (var bank = new GraphicsPath())
            {
                bank.StartFigure();
                bank.AddBezier(8, 185, 56, 162, 77, 191, 117, 183);
                bank.AddBezier(117, 183, 159, 173, 185, 155, 248, 177);
                bank.AddLine(248, 177, 248, 248);
                bank.AddLine(248, 248, 8, 248);
                bank.AddLine(8, 248, 8, 185);
                bank.CloseFigure();
                using (var snow = new LinearGradientBrush(new Point(115, 170), new Point(128, 248),
                    Color.FromArgb(255, 250, 253, 255), Color.FromArgb(255, 180, 215, 239)))
                    g.FillPath(snow, bank);
            }
            using (var shine = new Pen(Color.FromArgb(210, 255, 255, 255), 4) { StartCap = LineCap.Round, EndCap = LineCap.Round })
                g.DrawBezier(shine, 26, 201, 64, 180, 76, 207, 112, 198);
        }
        return bmp;
    }

    public static void Generate(string icoPath, string previewPath)
    {
        int[] sizes = { 16, 24, 32, 48, 64, 128, 256 };
        var frames = new List<byte[]>();
        foreach (int size in sizes)
        {
            using (var bmp = Render(size))
            using (var ms = new MemoryStream())
            {
                bmp.Save(ms, ImageFormat.Png);
                frames.Add(ms.ToArray());
                if (size == 256) bmp.Save(previewPath, ImageFormat.Png);
            }
        }
        using (var output = new BinaryWriter(File.Create(icoPath)))
        {
            output.Write((ushort)0); output.Write((ushort)1); output.Write((ushort)sizes.Length);
            int offset = 6 + sizes.Length * 16;
            for (int i = 0; i < sizes.Length; i++)
            {
                int size = sizes[i]; byte[] png = frames[i];
                output.Write((byte)(size == 256 ? 0 : size)); output.Write((byte)(size == 256 ? 0 : size));
                output.Write((byte)0); output.Write((byte)0); output.Write((ushort)1); output.Write((ushort)32);
                output.Write(png.Length); output.Write(offset); offset += png.Length;
            }
            foreach (byte[] png in frames) output.Write(png);
        }
    }
}
'@

$assets = $PSScriptRoot
[SnowIconGenerator]::Generate((Join-Path $assets 'SnowOverlay.ico'), (Join-Path $assets 'SnowOverlay-preview.png'))
Write-Host "Generated SnowOverlay.ico"
