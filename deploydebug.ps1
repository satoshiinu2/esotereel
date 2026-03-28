
$dist = 'build';

copy target/debug/nomyoedit_gui_helper.dll $dist/;
copy build/x64/bin/libqt6advanceddockingd.dll $dist/;

cd $dist;
windeployqt --release --no-translations --compiler-runtime gui.exe