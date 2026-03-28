
$dist = 'dist';
if (Test-Path $dist) { Remove-Item $dist -Recurse -Force };
New-Item -ItemType Directory -Path $dist;

copy build/gui.exe $dist/;
copy target/debug/nomyoedit_gui_helper.dll $dist/;
copy build/x64/bin/libqt6advanceddocking.dll $dist/;

cd $dist;
windeployqt --release --no-translations --compiler-runtime gui.exe