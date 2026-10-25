module Select

#if WINDOWS || UNIX
let platform = "known"
#else
let platform = undefinedPlatform
#endif
