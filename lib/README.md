# stine-rs
A library for STINE

Right now I would not consider this library usable.
The first think to change this, probably is a better the cache implementation

## Cache 
To reduce requests to stine a cache is used.
The default cache location depends on your OS, but is likely to be your default cache location (/home/users/.cache on linux)
Currently only data related to STINE Submodules and modules is saved in corresponding json files (also depends on the chosen language)

## LazyLoaded
Also to reduce requests. Some methods require a laziness level parameter to reduce some requests. The data can later be lazily loaded.
The current implementation (LazyLoaded) is probably suboptimal and could be reworked.


## TODO
- think about the caches
- The best options is to download the html and then just check if the file is offline avail and parse it.

- longer running functions should perhaps return iterators, for better understanding of the progress idk?

- impl get_content function which returns just the html content
- => then check if the language is correct

- rework some of the parsing code

## Problems
when parsing and checking submodule id, check if the trailing parameters are also the same, except for -N0, N1, etc..


## Async???