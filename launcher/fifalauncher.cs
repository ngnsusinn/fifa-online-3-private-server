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

    static string GetDefaultArgs()
    {
        string lang = Environment.GetEnvironmentVariable("FO3_LANGUAGE");
        string region = Environment.GetEnvironmentVariable("FO3_REGION");
        if (string.IsNullOrEmpty(region))
        {
            region = (lang != null && lang.Equals("vie", StringComparison.OrdinalIgnoreCase)) ? "VN" : "SG";
        }
        string regionLower = region.ToLowerInvariant();
        return string.Format(
            "-QUICKMATCH/SERVER=127.0.0.1:28000 -QUICKMATCH/PROTOCOL=tcp " +
            "-QUICKMATCH/NOTISERVER=127.0.0.1:28001 -PUBLISHER/GARENA/TOKEN=testtoken " +
            "-LOCALE/PUBLISHING_REGION={0} -DUMPTRUCK/SERVER=127.0.0.1 " +
            "region {1} token testtoken",
            region.ToUpperInvariant(), regionLower
        );
    }

    [STAThread]
    static int Main(string[] args)
    {
        string exe = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "fifazf.exe");
        if (!File.Exists(exe))
            exe = Path.Combine(GameDir, "fifazf.exe");

        string gameArgs;
        if (args.Length == 0)
        {
            gameArgs = GetDefaultArgs();
        }
        else if (args.Length == 1 && (args[0].Equals("eng", StringComparison.OrdinalIgnoreCase) || args[0].Equals("en", StringComparison.OrdinalIgnoreCase)))
        {
            gameArgs = "-QUICKMATCH/SERVER=127.0.0.1:28000 -QUICKMATCH/PROTOCOL=tcp " +
                       "-QUICKMATCH/NOTISERVER=127.0.0.1:28001 -PUBLISHER/GARENA/TOKEN=testtoken " +
                       "-LOCALE/PUBLISHING_REGION=SG -DUMPTRUCK/SERVER=127.0.0.1 " +
                       "region sg token testtoken";
        }
        else if (args.Length == 1 && (args[0].Equals("vie", StringComparison.OrdinalIgnoreCase) || args[0].Equals("vn", StringComparison.OrdinalIgnoreCase)))
        {
            gameArgs = "-QUICKMATCH/SERVER=127.0.0.1:28000 -QUICKMATCH/PROTOCOL=tcp " +
                       "-QUICKMATCH/NOTISERVER=127.0.0.1:28001 -PUBLISHER/GARENA/TOKEN=testtoken " +
                       "-LOCALE/PUBLISHING_REGION=VN -DUMPTRUCK/SERVER=127.0.0.1 " +
                       "region vn token testtoken";
        }
        else
        {
            gameArgs = string.Join(" ", args.Select(a => a.Contains(" ") ? "\"" + a + "\"" : a));
        }

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
