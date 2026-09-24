#| --------------------------------------------------------------------------
                        Optional: Multi-use buttons

Perhaps one of the most useful features of KMonad, where a lot of work has
gone into, but also an area with many buttons that are ever so slightly
different. The naming and structuring of these buttons might change sometime
soon, but for now, this is what there is.

For the next section being able to talk about examples is going to be handy,
so consider the following scenario and mini-language that will be the same
between scenarios:

  - We have some button `foo` that will be different between scenarios
  - `foo` is bound to 'Esc' on the input keyboard
  - the letters a s d f are bound to themselves
  - Px signifies the press of button x on the keyboard
  - Rx signifies the release of said button
  - Tx signifies the sequential and near instantaneous press and release of x
  - 100 signifies 100ms pass

So for example:
  Tesc Ta:
    tap of 'Esc' (triggering `foo`), tap of 'a' triggering `a`
  Pesc 100 Ta Tb Resc:
    press of 'Esc', 100ms pause, tap of 'a', tap of 'b', release of 'Esc'

The `tap-next` button takes 2 buttons, one for tapping, one for holding, and
combines them into a single button. When pressed, if the next event is its own
release, we tap the 'tapping' button. In all other cases we first press the
'holding' button then we handle the event. Then when the `tap-next` gets
released, we release the 'holding' button.

So, using our mini-language, we set foo to:
  (tap-next x lsft)
Then:
  Tesc            -> x
  Tesc Ta         -> xa
  Pesc Ta Resc    -> A
  Pesc Ta Tr Resc -> AR

The `tap-hold` button is very similar to `tap-next` (a theme, trust me). The
difference lies in how the decision is made whether to tap or hold. A
`tap-hold` waits for a particular timeout, if the `tap-hold` is released
anywhere before that moment we execute a tap immediately. If the timeout
occurs and the `tap-hold` is still held, we switch to holding mode.

The additional feature of a `tap-hold` is that it pauses event-processing
until it makes its decision and then rolls back processing when the decision
has been made.

So, again with the mini-language, we set foo to:
  (tap-hold 200 x lsft) ;; Like tap-next, but with a 200ms timeout
Then:
  Tesc             -> x
  Tesc Ta          -> xa
  Pesc 300 Ta      -> A (the moment you press a)
  Pesc Ta 300      -> A (after 200 ms)
  Pesc Ta 100 Resc -> xa (both happening immediately on Resc)

The `tap-hold-next` button is a combination of the previous 2. Essentially,
think of it as a `tap-next` button, but it also switches to held after a
period of time. This is useful, because if you have a (tap-next ret ctl) for
example, and you press it thinking you want to press C-v, but then you change
your mind, you now cannot release the button without triggering a 'ret', that
you then have to backspace. With the `tap-hold-next` button, you simply
outwait the delay, and you're good. I see no benefit of `tap-next` over
`tap-hold-next` with a decent timeout value.

You can use the `:timeout-button` keyword to specify a button other than the
hold button which should be held when the timeout expires. For example, we
can construct a button which types one x when tapped, multiple x's when held,
and yet still acts as shift when another button is pressed before the timeout
expires. So, using the minilanguage and foo as:
  (tap-hold-next 200 x lsft :timeout-button x)
Then:
  Tesc           -> Tx
  Pesc 100 Ta    -> A (the moment you press a)
  Pesc 5000 Resc -> xxxxxxx (some number of auto-repeated x's)

Note that KMonad does not itself auto-repeat the key. In this last example,
KMonad emits 200 Px 4800 Rx, and the operating system's auto-repeat feature,
if any, emits multiple x's because it sees that the x key is held for 4800 ms.

A note about tap action duration:
For simplicity we reuse the `tap-next` example above, set foo to:
  (tap-next x lsft)
Now, any keystroke performed by baseline human will have some duration, a
'Tesc' is actually 'Pesc <some time passed> Resc'.  A true tap 'Tesc' with no
delay between the press and release will sometime experience registration
problems in programs.  However the tap action performed by KMonad IS this kind
of 'true tap', that is:
  Tesc (Pesc 100 Resc) -> Px Rx
For various reasons we do not want KMonad to have some default duration in the
tap action it performs.  If you are having issues in programs, you can instead
use the aforementioned `around` and `pause` function to give the tap action
some duration.  Set foo to:
  (tap-next (around x (pause 2000)) lsft)
or equivalently:
  (tap-next (around x P2000) lsft)
then we have:
  Tesc (Pesc 100 Resc) -> Px 2000 Rx
2000 ms is just for you to distinctively see the effect, in practice 35 ms
should be enough for most scenarios (slightly longer than 2 frames in 60 fps).

The `tap-next-release` is like `tap-next`, except it decides whether to tap or
hold based on the next release of a key that was *not* pressed before us. This
also performs rollback like `tap-hold`. So, using the minilanguage and foo as:
  (tap-next-release x lsft)
Then:
  Tesc Ta         -> xa
  Pa Pesc Ra Resc -> ax (because 'a' was already pressed when we started, so
                          foo decides it is tapping)
  Pesc Pa Resc Ra -> xa (because the first release we encounter is of esc)
  Pesc Ta Resc    -> A (because a was pressed *and* released after we started,
                        so foo decides it is holding)

`tap-next-press` is also a lot like `tap-next`, but decides whether to tap or
hold based on whether another key is pressed before this one is released.
Using the minilanguage:
  (tap-next-press x lsft)
Then:
  Tesc Ta -> xa
  Pa Pesc Ra Resc -> ax (because esc is released before another key is pressed)
  Pesc Pa Resc Ra -> A (because a is pressed before esc is released)
  Pesc Ta Resc    -> A (a is pressed before esc is released here as well)

It also has a hold variant named `tap-hold-next-press` (notice a trend?).
It works just like `tap-next-press` except that after the timeout it will
jump into holding-mode. The holding button for the timeout case can be swapped
out just like `tap-hold-next` via a `:timeout-button ...` as the last argument.
(since 0.4.4)

These increasingly stranger buttons are, I think, coming from the stubborn
drive of some of my more eccentric (and I mean that in the most positive way)
users to make typing with modifiers on the home-row more comfortable.
Especially layouts that encourage a lot of rolling motions are nicer to use
with the `release` style buttons.

The `tap-hold-next-release` is just like `tap-next-release`,
but it comes with an additional timeout that, just like `tap-hold-next` will
jump into holding-mode after a timeout.

I honestly think that `tap-hold-next-release`, although it seems the most
complicated, probably is the most comfortable to use. But I've put all of them
in a testing layer down below, so give them a go and see what is nice.

-------------------------------------------------------------------------- |#