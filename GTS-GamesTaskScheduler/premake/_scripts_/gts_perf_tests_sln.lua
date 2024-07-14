-- premake5.lua

workspace "gts_perf_tests"
    configurations { "Debug", "DebugWithCounting","DebugWithLogger", "DebugWithTracy", "RelWithCounting", "RelWithLogger", "RelWithTracy", "RelWithSimTrace", "Release" }
    architecture "x86_64"
    cppdialect "C++17"

    
    location ("../../_build/gts_perf_tests_" .. _ACTION .. (_ARGS[1] and ("_" .. _ARGS[1]) or ("")))
    startproject "gts_perf_tests"
    
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
        defines { "_DEBUG"}
        symbols "On"
        
    filter {"configurations:DebugWithCounting"}
        defines { "_DEBUG", "GTS_ENABLE_COUNTER=1"}
        symbols "On"
        
    filter { "configurations:DebugWithLogger" }
        defines { "_DEBUG", "GTS_TRACE_CONCURRENT_USE_LOGGER=1" }
        symbols "On"

    filter { "configurations:DebugWithTracy" }
        defines { "_DEBUG", "GTS_TRACE_USE_TRACY=1", "TRACY_ENABLE" }
        symbols "On"
        
    filter { "configurations:RelWithCounting" }
        defines { "NDEBUG", "GTS_ENABLE_COUNTER=1" }
        symbols "On"
        optimize "Full"

    filter { "configurations:RelWithLogger" }
        defines { "NDEBUG", "GTS_TRACE_CONCURRENT_USE_LOGGER=1" }
        symbols "On"
        optimize "Full"

    filter { "configurations:RelWithTracy" }
        defines { "NDEBUG", "GTS_TRACE_USE_TRACY=1", "TRACY_ENABLE" }
        symbols "On"
        optimize "Full"
        
    filter { "configurations:RelWithSimTrace" }
        defines { "NDEBUG", "GTS_ENABLE_SIM_TRACE" }
        symbols "On"
        optimize "Full"

    filter { "configurations:Release" }
        defines { "NDEBUG", "GTS_USE_MEM_DEBUG" }
        optimize "Full"

    filter { "configurations:ReleaseAnalyze" }
        defines { "NDEBUG", "GTS_ANALYZE" }
        optimize "Full"

    include "_intermediates_/gts_tracy"
    include "_intermediates_/tracy"

project "gts_perf_tests"
    
    kind "ConsoleApp"
    language "C++"
    targetdir "%{prj.location}/%{cfg.buildcfg}_%{cfg.architecture}"
    links { "gts", "tracy" }
    includedirs {
        "../../external_dependencies/tracy",
        "../../source/gts/include",
        "../../source/gts/test_perf/include"
    }
    files {
        "../../source/gts/test_perf/include/**.*",
        "../../source/gts/test_perf/source/**.*"
    }
