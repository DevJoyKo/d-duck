## Windows 미리보기 (x64)

서명하지 않은 미리보기 설치 파일이에요. Windows 10/11 x64에서 동작해요.

### 설치할 때 SmartScreen 경고가 나오면
1. "Windows의 PC 보호" 창에서 **추가 정보**를 눌러요.
2. 게시자가 "알 수 없는 게시자"인지 확인하고 **실행**을 눌러요.
3. 설치는 사용자 폴더(`%LOCALAPPDATA%\D.Duck`)에만 되고 관리자 권한을 요구하지 않아요.

### 받은 파일 확인하기
```powershell
Get-FileHash .\D.Duck_*_x64-setup.exe -Algorithm SHA256   # SHA256SUMS-windows.txt와 같아야 해요
gh attestation verify .\D.Duck_*_x64-setup.exe --repo <owner>/<repo>
```
`gh attestation verify`가 통과하면 이 저장소의 릴리스 워크플로가 태그에서 만든 파일이에요.

### Windows에서 Mac 전용인 기능
로컬만(원격 금지) 작업, 남은 Claude 프로세스 강제 종료, 뉴스 가져오기, 사용량 인사이트, 브리프 다듬기, 클라우드 루틴, 리포트 쓰기 허용은 Windows에서 막혀 있고 "Mac 전용"으로 보여요. Windows CLI에서 훅이 실제로 막히는지 확인한 뒤에 풀어요.

### 실제 Windows에서 아직 확인하지 못한 것 ([manual])
- 백그라운드 작업이 앱을 닫아도 계속 돌고, 다시 열면 보여요.
- claude·git을 실행할 때 콘솔 창이 뜨지 않아요.
- PowerShell이나 Bash로 `git push origin main`을 시키면 거부돼요.
- Git for Windows가 없으면 작성 화면이 안내하고, 작업에 셸 도구가 없어요.
- 받은 설치 파일을 `gh attestation verify`로 확인할 수 있어요.
- 새로 설치한 Windows 11에서 시작 메뉴에 나타나고 두 번째 실행은 기존 창을 앞으로 가져와요.
- 프로젝트 목록·작성 폴더·외부 링크·휴지통(Recycle Bin)·터미널 열기·트레이 아이콘이 실제 화면에서 맞게 보여요.
