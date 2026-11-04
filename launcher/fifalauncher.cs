// Replacement for the Garena fifalauncher.exe (Themida-packed; its Garena SSO / version
// servers are dead). Does what the original does at the end of its flow: start fifazf.exe
// with server args and stay alive, because the game is handed the launcher PID.
// Anti-cheat is untouched: fifazf.exe still runs its own XignCode/HackShield init.
//
// Build: C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe /nologo /target:winexe /out:fifalauncher.exe fifalauncher.cs
using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Windows.Forms;

static class Launcher
{
    const string GameDir = @"C:\Users\ngnsusinn\Documents\fifa-online-3-server\FO3VN";

    static string BuildGameArgs(string[] args)
    {
        // If caller passed full arguments with custom QUICKMATCH/SERVER, respect them.
        if (args != null && args.Any(a => a.IndexOf("QUICKMATCH/SERVER", StringComparison.OrdinalIgnoreCase) >= 0))
        {
            return string.Join(" ", args.Select(a => a.Contains(" ") ? "\"" + a + "\"" : a));
        }

        string lang = Environment.GetEnvironmentVariable("FO3_LANGUAGE");
        string region = Environment.GetEnvironmentVariable("FO3_REGION");
        string token = "testtoken";
        string server = "127.0.0.1:28000";
        string notiServer = "127.0.0.1:28001";

        if (args != null && args.Length > 0)
        {
            for (int i = 0; i < args.Length; i++)
            {
                string a = args[i];
                if (a.Equals("eng", StringComparison.OrdinalIgnoreCase) || a.Equals("en", StringComparison.OrdinalIgnoreCase) || a.Equals("sg", StringComparison.OrdinalIgnoreCase))
                {
                    lang = "eng";
                    region = "SG";
                }
                else if (a.Equals("vie", StringComparison.OrdinalIgnoreCase) || a.Equals("vn", StringComparison.OrdinalIgnoreCase))
                {
                    lang = "vie";
                    region = "VN";
                }
                else if (a.Equals("ind", StringComparison.OrdinalIgnoreCase) || a.Equals("id", StringComparison.OrdinalIgnoreCase))
                {
                    lang = "ind";
                    region = "ID";
                }
                else if (a.StartsWith("-LOCALE/PUBLISHING_REGION=", StringComparison.OrdinalIgnoreCase))
                {
                    region = a.Substring("-LOCALE/PUBLISHING_REGION=".Length);
                }
                else if (a.Equals("region", StringComparison.OrdinalIgnoreCase) && i + 1 < args.Length)
                {
                    region = args[++i];
                }
                else if (a.StartsWith("-PUBLISHER/GARENA/TOKEN=", StringComparison.OrdinalIgnoreCase))
                {
                    token = a.Substring("-PUBLISHER/GARENA/TOKEN=".Length);
                }
                else if (a.Equals("token", StringComparison.OrdinalIgnoreCase) && i + 1 < args.Length)
                {
                    token = args[++i];
                }
            }
        }

        if (string.IsNullOrEmpty(region))
        {
            if (lang != null && (lang.Equals("vie", StringComparison.OrdinalIgnoreCase) || lang.Equals("vn", StringComparison.OrdinalIgnoreCase)))
            {
                region = "VN";
            }
            else if (lang != null && (lang.Equals("ind", StringComparison.OrdinalIgnoreCase) || lang.Equals("id", StringComparison.OrdinalIgnoreCase)))
            {
                region = "ID";
            }
            else
            {
                region = "SG";
            }
        }

        string regionUpper = region.ToUpperInvariant();
        string regionLower = region.ToLowerInvariant();

        return string.Format(
            "-QUICKMATCH/SERVER={0} -QUICKMATCH/PROTOCOL=tcp " +
            "-QUICKMATCH/NOTISERVER={1} -PUBLISHER/GARENA/TOKEN={2} " +
            "-LOCALE/PUBLISHING_REGION={3} -DUMPTRUCK/SERVER=127.0.0.1 " +
            "region {4} token {2}",
            server, notiServer, token, regionUpper, regionLower
        );
    }

    [STAThread]
    static int Main(string[] args)
    {
        string exe = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "fifazf.exe");
        if (!File.Exists(exe))
            exe = Path.Combine(GameDir, "fifazf.exe");

        string gameArgs = BuildGameArgs(args);

        gameArgs += " -FIFA/LAUNCHER/PID=" + Process.GetCurrentProcess().Id;

        var psi = new ProcessStartInfo(exe, gameArgs)
        {
            WorkingDirectory = Path.GetDirectoryName(exe),
            UseShellExecute = true,
        };

        try
        {
            using (var game = Process.Start(psi))
            {
                if (game != null)
                {
                    game.WaitForExit();
                    return game.ExitCode;
                }
                return 1;
            }
        }
        catch (Exception e)
        {
            MessageBox.Show(exe + "\n\n" + e.Message, "fifalauncher", MessageBoxButtons.OK, MessageBoxIcon.Error);
            return 1;
        }
    }
}
