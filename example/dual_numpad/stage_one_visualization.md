# Dual numpad input (rotated setting)
Rotated in this way, the top rightmost key and the bottom leftmost key are
missing, because physical KPENTER key span two spaces.

## Left keyboard
```
KP0     KP1     KP4     KP7     NUMLOCK _       _       _       _       _       _       
SPACE   KP2     KP5     KP8     KPSLASH TAB     _       _       _       _       _       _       
KPDOT   KP3     KP6     KP9     KPAST   _       _       _       _       _       _       _       
        KPENTER KPPLUS  KPMINUS BCKSPC  _       _       _       _       _       _       _       
```

## Left consumer
```
_       _       _       _       _       HOMEPG  _       _       _       _       _       
_       _       _       _       _       _       _       _       _       _       _       _       
_       _       _       _       _       MAIL    _       _       _       _       _       _       
        _       _       _       _       CALC    _       _       _       _       _       _       
```

## Right keyboard
```
_       _       _       _       _       _       _       BCKSPC  KPMINUS KPPLUS  KPENTER 
_       _       _       _       _       _       _       KPAST   KP9     KP6     KP3     KPDOT   
_       _       _       _       _       _       TAB     KPSLASH KP8     KP5     KP2     SPACE   
        _       _       _       _       _       _       NUMLOC  KP7     KP4     KP1     KP0     
```

## Right consumer
```
_       _       _       _       _       _       CALC    _       _       _       _       
_       _       _       _       _       _       MAIL    _       _       _       _       _       
_       _       _       _       _       _       _       _       _       _       _       _       
        _       _       _       _       _       HOMEPG  _       _       _       _       _       
```


# Target for stage one
All pysical devices should map to the same intermediate layout, so layers on top
on that always feels the same. This is the blueprint:
```
TAB     Q       W       F       P       G       J       L       U       Y       SEMICLN DELETE  
BACKSPC A       R       S       T       D       H       N       E       I       O       ENTER   
LSHIFT  Z       X       C       V       B       K       M       COMMA   DOT     SLASH   RSHIFT  
CAPSLCK LCTRL   LMETA   LALT     _______SPACE__________________ RALT    RMETA   RCTRL   ESC     
```
Depending on the physical device used for typing, some of the keys may be dead:
plan replacement in the hold layers.
