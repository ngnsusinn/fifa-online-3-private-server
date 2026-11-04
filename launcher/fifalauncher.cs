// Replacement for the Garena fifalauncher.exe.
// Starts fifazf.exe with server arguments and redirects APSEODESK172 to 127.0.0.1 directly in memory,
// so no Windows hosts file configuration is needed.
//
// Build: C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe /nologo /target:winexe /out:fifalauncher.exe fifalauncher.cs
using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Windows.Forms;

static class Launcher
{
    const string GameDir = @"C:\Users\ngnsusinn\Documents\fifa-online-3-server\FO3VN";
    const string Defaults =
        "-QUICKMATCH/SERVER=127.0.0.1:28000 -QUICKMATCH/PROTOCOL=tcp " +
        "-QUICKMATCH/NOTISERVER=127.0.0.1:28001 -PUBLISHER/GARENA/TOKEN=testtoken " +
        "-LOCALE/PUBLISHING_REGION=VN -DUMPTRUCK/SERVER=127.0.0.1 " +
        "region vn token testtoken";

    [StructLayout(LayoutKind.Sequential)]
    struct MEMORY_BASIC_INFORMATION
    {
        public IntPtr BaseAddress;
        public IntPtr AllocationBase;
        public uint AllocationProtect;
        public ushort PartitionId;
        public IntPtr RegionSize;
        public uint State;
        public uint Protect;
        public uint Type;
    }

    const uint PROCESS_VM_READ = 0x0010;
    const uint PROCESS_VM_WRITE = 0x0020;
    const uint PROCESS_QUERY_INFORMATION = 0x0400;

    const uint MEM_COMMIT = 0x1000;
    const uint PAGE_NOACCESS = 0x01;
    const uint PAGE_GUARD = 0x100;

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern IntPtr OpenProcess(uint dwDesiredAccess, bool bInheritHandle, int dwProcessId);

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern int VirtualQueryEx(IntPtr hProcess, IntPtr lpAddress, out MEMORY_BASIC_INFORMATION lpBuffer, uint dwLength);

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool ReadProcessMemory(IntPtr hProcess, IntPtr lpBaseAddress, [Out] byte[] lpBuffer, int dwSize, out IntPtr lpNumberOfBytesRead);

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool WriteProcessMemory(IntPtr hProcess, IntPtr lpBaseAddress, byte[] lpBuffer, int nSize, out IntPtr lpNumberOfBytesWritten);

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool CloseHandle(IntPtr hObject);

    static void PatchHostInMemory(int pid)
    {
        IntPtr hProcess = IntPtr.Zero;
        for (int retry = 0; retry < 20; retry++)
        {
            hProcess = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_VM_WRITE, false, pid);
            if (hProcess != IntPtr.Zero)
                break;
            Thread.Sleep(50);
        }

        if (hProcess == IntPtr.Zero)
            return;

        try
        {
            byte[] targetBytes = Encoding.ASCII.GetBytes("APSEODESK172\0");
            byte[] replaceBytes = Encoding.ASCII.GetBytes("127.0.0.1\0\0\0\0");
            int mbiSize = Marshal.SizeOf(typeof(MEMORY_BASIC_INFORMATION));
            long maxAddr = 0x7FFFFFFF0000L;

            DateTime start = DateTime.UtcNow;
            while ((DateTime.UtcNow - start).TotalSeconds < 8.0)
            {
                long addr = 0;
                bool patched = false;

                while (addr < maxAddr)
                {
                    MEMORY_BASIC_INFORMATION mbi;
                    if (VirtualQueryEx(hProcess, (IntPtr)addr, out mbi, (uint)mbiSize) == 0)
                        break;

                    long regionSize = mbi.RegionSize.ToInt64();
                    if (mbi.State == MEM_COMMIT && (mbi.Protect & (PAGE_NOACCESS | PAGE_GUARD)) == 0 && regionSize > 0 && regionSize < (64 * 1024 * 1024))
                    {
                        byte[] buffer = new byte[(int)regionSize];
                        IntPtr bytesRead;
                        if (ReadProcessMemory(hProcess, mbi.BaseAddress, buffer, buffer.Length, out bytesRead))
                        {
                            int readLen = bytesRead.ToInt32();
                            for (int i = 0; i <= readLen - targetBytes.Length; i++)
                            {
                                if (buffer[i] == targetBytes[0] && MatchBytes(buffer, i, targetBytes))
                                {
                                    IntPtr writeAddr = (IntPtr)(mbi.BaseAddress.ToInt64() + i);
                                    IntPtr written;
                                    WriteProcessMemory(hProcess, writeAddr, replaceBytes, replaceBytes.Length, out written);
                                    patched = true;
                                }
                            }
                        }
                    }

                    addr = mbi.BaseAddress.ToInt64() + regionSize;
                }

                if (patched)
                    break;

                Thread.Sleep(30);
            }
        }
        catch
        {
        }
        finally
        {
            CloseHandle(hProcess);
        }
    }

    static bool MatchBytes(byte[] buffer, int offset, byte[] target)
    {
        for (int j = 0; j < target.Length; j++)
        {
            if (buffer[offset + j] != target[j])
                return false;
        }
        return true;
    }

    [STAThread]
    static int Main(string[] args)
    {
        string exe = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "fifazf.exe");
        if (!File.Exists(exe))
            exe = Path.Combine(GameDir, "fifazf.exe");

        string gameArgs = args.Length == 0
            ? Defaults
            : string.Join(" ", args.Select(a => a.Contains(" ") ? "\"" + a + "\"" : a));
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
                    int gamePid = game.Id;
                    ThreadPool.QueueUserWorkItem(_ => PatchHostInMemory(gamePid));
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
