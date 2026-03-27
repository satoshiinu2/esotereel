
$dist = 'gui/dist';
if (Test-Path $dist) { Remove-Item -Recycle $dist -Recurse -Force };
New-Item -ItemType Directory -Path $dist;

copy gui/build/gui.exe $dist/;
copy target/debug/nomyoedit_gui_helper.dll $dist/;
copy gui/build/x64/bin/libqt6advanceddockingd.dll $dist/;

cd $dist;
windeployqt --release --no-translations --compiler-runtime gui.exe