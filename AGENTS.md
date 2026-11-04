# Agent Rules & Guidelines

Tài liệu này quy định các nguyên tắc làm việc bắt buộc dành cho các AI Agent (Hermes, Claude, Copilot, v.v.) khi phát triển và bảo trì dự án **FIFA Online 3 Private Server**.

---

## 1. Quy tắc Git & Đồng bộ mã nguồn (Commit & Push)

- **Tự động Commit & Push sau mỗi lần chỉnh sửa:**
  - Sau mỗi lần sửa lỗi, tái cấu trúc hoặc thêm tính năng mới, Agent **bắt buộc phải tạo commit** với thông điệp rõ ràng (theo chuẩn [Conventional Commits](https://www.conventionalcommits.org/): `fix:`, `feat:`, `refactor:`, `test:`, `docs:`).
  - Ngay sau khi commit, **phải thực hiện push ngay lên GitHub** (`git push origin <branch>`).
  - Không để các thay đổi tồn đọng dạng uncommitted/unpushed qua các lượt hội thoại.
- **Giữ sạch Repository:**
  - Tuyệt đối không commit các file dữ liệu game cá nhân, file nhị phân lớn hoặc thư mục được liệt kê trong `.gitignore` (`FO3VN/`, `orig_source/`, `gamedata/`, `captures/`, `certs/`, `target/`).

---

## 2. Quy tắc Biên dịch & Build Binary (Release)

- **Biên dịch toàn bộ workspace:**
  - Khi thay đổi mã nguồn trong `src/` (như `src/http/`, `src/blaze/`, `src/messenger/`), `start_all.exe` là tiến trình cha gọi các file nhị phân con (`http_config.exe`, `server.exe`, `messenger.exe`).
  - Do đó, **bắt buộc phải chạy `cargo build --release`** (không dùng `--bin start_all` đơn lẻ) để đảm bảo toàn bộ các file `.exe` con trong `target/release/` đều được cập nhật đồng bộ.
- **Biên dịch Launcher C# (`launcher/fifalauncher.cs`):**
  - Khi chỉnh sửa `launcher/fifalauncher.cs`, phải biên dịch lại bằng `csc.exe` và copy đè file mới vào thư mục client:
    ```cmd
    cd launcher && C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe /nologo /target:winexe /out:fifalauncher.exe fifalauncher.cs
    copy /y launcher\fifalauncher.exe FO3VN\fifalauncher.exe
    ```

---

## 3. Quy tắc Kiểm thử (Testing)

- **Đảm bảo không hồi quy:** Trước khi commit, phải chạy `cargo test` và đảm bảo toàn bộ unit tests và integration tests đều đạt **PASS 100%**.
- **Viết test cho endpoint mới:** Khi sửa hoặc thêm các endpoint API trong HTTP router hoặc Blaze server, phải bổ sung test case tương ứng vào `tests/admin_and_http_tests.rs` hoặc `tests/blaze_loopback_tests.rs`.

---

## 4. Quy tắc Tương thích Game Client & Protocol

- **Hỗ trợ đa ngôn ngữ / Region:**
  - Mặc định ưu tiên cấu hình Tiếng Anh (`FO3_LANGUAGE=eng`, `FO3_REGION=SG`).
  - Hỗ trợ Tiếng Việt (`FO3_LANGUAGE=vie`, `FO3_REGION=VN`) và Tiếng Indonesia (`FO3_LANGUAGE=ind`, `FO3_REGION=ID`).
  - Cấu hình launcher và server phải đồng bộ mã vùng (`region sg` đi kèm `-LOCALE/PUBLISHING_REGION=SG` để client load đúng file cấu hình `locale@sg.ini` và asset `language_eng.big`).
- **Độ chính xác của cấu trúc dữ liệu ActionScript (Flash):**
  - Flash client rất nhạy cảm với lỗi `null` / `undefined` (Error #1009, Error #1010):
    - Endpoint `/account` phải luôn có đầy đủ trường `option` và `options` (bao gồm `soundOption`, `controlOption`, `screenOption`).
    - Endpoint `/coaches` phải trả về đúng mảng `coachlist` (mỗi HLV phải có `summary`, `teamvalue`, `levelinfo`, `state`).
    - Endpoint `/coach/<slot>/default` và `/coach/me/onready` phải luôn trả về mảng `resources` chứa tài nguyên `/coach/me` với đầy đủ `id` và cấu trúc `teamsquadlist`.
    - Endpoint `/game/inweburl/<name>` phải trả về `{"result": true, "url": "about:blank"}` để tránh crash `LoadWebAddress` chuỗi rỗng.
