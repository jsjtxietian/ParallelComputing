@REM ..\windows\premake5.exe --file=gts_examples_sln.lua vs2022 clang

cd ..\_scripts_
..\windows\premake5.exe --file=gts_sln.lua vs2022 msvc
..\windows\premake5.exe --file=gts_examples_sln.lua vs2022 msvc
..\windows\premake5.exe --file=gts_perf_tests_sln.lua vs2022 msvc
..\windows\premake5.exe --file=gts_unit_tests_sln.lua vs2022 msvc