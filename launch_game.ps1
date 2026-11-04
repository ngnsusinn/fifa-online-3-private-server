# Launch the FO3ID client pointed at the local servers (bypasses the Garena launcher).
#
# NOTE: this still requires XignCode to pass first. With the Garena update server dead,
# the client currently exits before the network stage, so it will NOT yet reach
# 127.0.0.1:28000. Once XignCode is unblocked (real update files served locally), this
# is how you point the game at the local Blaze server.
#
# Start the servers first:  target\release\start_all.exe (or play.bat)
# Then run this (XignCode loads a kernel driver, so run as Administrator).

param(
  [string]$Exe = 'C:\Users\ngnsusinn\Documents\fifa-online-3-server\FO3VN\fifazf.exe',
  [string]$GameServer = '127.0.0.1:28000',
  [string]$NotiServer = '127.0.0.1:28001'
)

& $Exe `
  "-QUICKMATCH/SERVER=$GameServer" `
  "-QUICKMATCH/PROTOCOL=tcp" `
  "-QUICKMATCH/NOTISERVER=$NotiServer" `
  "-FIFA/LAUNCHER/PID=$PID" `
  "-PUBLISHER/GARENA/TOKEN=testtoken" `
  "-LOCALE/PUBLISHING_REGION=VN" `
  "-DUMPTRUCK/SERVER=127.0.0.1" `
  region vn token testtoken
# ^ alias pairs (separate argv, no dash) parsed at fifazf+0x1EEE70: `region` must be a
#   lowercase Garena code (sg/my/th/vn/id) and `token` must be set, else the client takes
#   the Tencent path and fails "passport/token/signature missing".
