-- premake5.lua

workspace "gts_malloc"
    configurations { "Debug", "RelWithAssert", "Release" }
    architecture "x86_64"
    cppdialect "C++17"

    
    location ("../../_build/gts_malloc/" .. _ACTION .. (_ARGS[1] and ("/" .. _ARGS[1]) or ("")))
    startproject "gts_malloc"
    
    warnings "Extra"
    exceptionhandling "Off"
    rtti "Off"
    
        
    filter { "action:vs*" }
        defines { "_HAS_EXCEPTIONS=0" }
        linkoptions { "-IGNORE:4099", "-IGNORE:4221" }
        systemversion "latest"
        
    filter { "action:gmake" }
        buildoptions { "-pedantic", "-Wno-error=class-memaccess", "-msse2" }
        
    filter { "action:xcode4" }
        buildoptions { "-pedantic", "-Wno-error=class-memaccess", "-msse2" }
       
    filter {"configurations:Debug"}
        defines { "_DEBUG" }
        symbols "On"

    filter { "configurations:RelWithAssert" }
        defines { "NDEBUG", "GTS_USE_ASSERTS", "GTS_USE_INTERNAL_ASSERTS" }
        symbols "On"
        optimize "Full"

    filter { "configurations:Release" }
        defines { "NDEBUG" }
        symbols "On"
        optimize "Full"
 
    include "_intermediates_/gts_malloc_redirect"
    include "_intermediates_/gts_malloc_static"
    include "_intermediates_/gts_malloc_shared"
