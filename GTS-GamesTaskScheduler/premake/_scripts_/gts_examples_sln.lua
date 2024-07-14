-- premake5.lua

workspace "gts_examples"
    configurations {  "Debug", "DebugWithInstrument", "RelWithInstrument", "RelWithCounting", "Release" }
    architecture "x86_64"
    cppdialect "C++17"
    
    location ("../../_build/gts_examples_" .. _ACTION .. (_ARGS[1] and ("_" .. _ARGS[1]) or ("")))
    startproject "fib_example"
    
    --warnings "Extra"
    exceptionhandling "Off"
    rtti "Off"
    
        
    filter { "action:vs*" }        
        defines { "_HAS_EXCEPTIONS=0", "_CRT_SECURE_NO_WARNINGS" }
        linkoptions { "-IGNORE:4221" }
        systemversion "latest"
               
    filter { "action:gmake" }
        buildoptions { "-pedantic", "-Wno-class-memaccess", "-msse2" }
        
    filter { "action:xcode4" }
        buildoptions { "-pedantic", "-Wno-class-memaccess", "-msse2" }
       
    filter {"configurations:Debug"}
        symbols "On"
        defines { "_DEBUG" }
        
    filter { "configurations:DebugWithInstrument" }
        defines { "_DEBUG", "GTS_ENABLE_INSTRUMENTER", "GTS_ENABLE_CONCRT_LOGGER", "GTS_ENABLE_CONCRT_LOGGER" }
        symbols "On"

    filter { "configurations:RelWithInstrument" }
        defines { "NDEBUG", "GTS_ENABLE_INSTRUMENTER", "GTS_ENABLE_CONCRT_LOGGER" }
        symbols "On"
        optimize "Full"
        
    filter { "configurations:RelWithCounting" }
        defines { "NDEBUG", "GTS_ENABLE_COUNTER=1" }
        symbols "On"
        optimize "Full"

    filter { "configurations:Release" }
        defines { "NDEBUG" }
        optimize "Full"
        
    include "_intermediates_/gts"
    
group "0_quick_start"
    include "_intermediates_/gts_ex_quick_start"

group "1_micro_scheduler"
    include "_intermediates_/gts_ex_micro_scheduler"

group "3_macro_scheduler"
    include "_intermediates_/gts_ex_macro_scheduler"
