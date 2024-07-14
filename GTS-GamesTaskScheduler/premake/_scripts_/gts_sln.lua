-- premake5.lua

workspace "gts"
    configurations { "Debug", "RelWithAssert", "Release" }
    architecture "x86_64"
    cppdialect "C++17"
    
    location ("../../_build/gts_" .. _ACTION .. (_ARGS[1] and ("_" .. _ARGS[1]) or ("")))
    startproject "gts"
    
    warnings "Extra"
    exceptionhandling "Off"
    rtti "Off"
    
    filter { "action:vs*" }        
        defines { "_HAS_EXCEPTIONS=0" }
        linkoptions { "-IGNORE:4221" }
        systemversion "latest"
        
    filter { "action:gmake" }
        buildoptions { "-pedantic", "-Wno-error=class-memaccess", "-msse2" }
        
    filter { "action:xcode4" }
        buildoptions { "-pedantic", "-Wno-error=class-memaccess", "-msse2" }
       
    filter {"configurations:Debug"}
        defines { "_DEBUG" }
        symbols "On"

    filter { "configurations:RelWithDebInfo" }
        defines { "NDEBUG", "GTS_USE_FATAL_ASSERT" }
        symbols "On"
        optimize "Full"

    filter { "configurations:Release" }
        defines { "NDEBUG" }
        symbols "On"
        optimize "Full"
 
    include "_intermediates_/gts"
