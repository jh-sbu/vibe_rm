Scriptname VrmSaveLoadScript extends Quest
; test-mod saveload: what a save and its load keep of NPCs busy at save time
; (furniture, sleep, sandbox idles, sneaking, a fight, UseMagic practice,
; a Travel walk) and of a script's looping sound. See saveload.rs.

Actor Property Player Auto
Actor Property Sitter Auto
Actor Property Sleeper Auto
Actor Property Sandboxer Auto
Actor Property Sneaker Auto
Actor Property Fighter Auto
Actor Property Target Auto
Actor Property Caster Auto
Actor Property Walker Auto
ObjectReference Property Entry Auto
ObjectReference Property Chair Auto
ObjectReference Property Bed Auto
ObjectReference Property WalkerHome Auto
ObjectReference Property WalkerFar Auto
GlobalVariable Property Go Auto
GlobalVariable Property SkipTest Auto
Sound Property Loop Auto
Spell Property Flames Auto
Weapon Property Sword Auto
Message Property AskHear Auto
Message Property AskHeardAfter Auto
Message Property AskStopped Auto
Package Property PkgSit Auto
Package Property PkgSleep Auto
Package Property PkgSandbox Auto
Package Property PkgCast Auto
Package Property PkgWalkFar Auto
Package Property PkgWalkHome Auto
Package Property PkgSneakHome Auto
Package Property PkgFightHome Auto
Package Property PkgTargetHome Auto

String ResultSetup
String ResultBefore
String ResultBeforeGraph
String ResultSave
String ResultLoad0
String ResultLoad0Graph
String ResultLoad2
String ResultLoad10
String ResultLoad10Graph
String ResultSound

Bool Ran = false
Actor[] Actors
String[] Names
Float[] SavedX
Float[] SavedY
Float[] SavedZ
Float[] SavedA
Int LoopId = 0
Float TSave = 0.0
Float TLoad = 0.0
Bool Loaded = false

Event OnInit()
    RegisterForSingleUpdate(5.0)
EndEvent

Event OnUpdate()
    If Ran
        return
    EndIf
    Ran = true
    Actors = new Actor[8]
    Actors[0] = Sitter
    Actors[1] = Sleeper
    Actors[2] = Sandboxer
    Actors[3] = Sneaker
    Actors[4] = Fighter
    Actors[5] = Target
    Actors[6] = Caster
    Actors[7] = Walker
    Names = new String[8]
    Names[0] = "Sitter"
    Names[1] = "Sleeper"
    Names[2] = "Sandboxer"
    Names[3] = "Sneaker"
    Names[4] = "Fighter"
    Names[5] = "Target"
    Names[6] = "Caster"
    Names[7] = "Walker"
    SavedX = new Float[8]
    SavedY = new Float[8]
    SavedZ = new Float[8]
    SavedA = new Float[8]
    Debug.MessageBox("vibe_rm saveload: you will be moved to a test cell where eight NPCs are put into one state each. Leave the game alone until a box asks you to load a save (2 to 3 minutes). Three boxes ask whether you hear a looping crowd sound; answer them by ear.")
    Utility.Wait(1.0)

    Announce(1, "Setup")
    If SkipTest.GetValue() as Int == 1
        ResultSetup = "skipped by SkipTest\n"
    Else
        ResultSetup = Setup()
    EndIf
    Debug.Trace("vibe_rm saveload Setup: " + ResultSetup)
    Checkpoint()

    ; From here to the load nothing saves: the walker is caught halfway.
    Announce(2, "Before")
    ResultBefore = Before()
    Debug.Trace("vibe_rm saveload Before: " + ResultBefore)
    ResultBeforeGraph = Graphs("at the save request")
    ResultSave = SaveAndLoad()
    Debug.Trace("vibe_rm saveload Save: " + ResultSave)

    ; Straight after the load: no waits but these.
    ResultLoad0 = "load + " + Num(Now() - TLoad) + " s:\n" + States()
    ResultLoad0Graph = Graphs("load + " + Num(Now() - TLoad) + " s")
    WaitSince(TLoad, 2.0)
    ResultLoad2 = "load + " + Num(Now() - TLoad) + " s:\n" + States()
    WaitSince(TLoad, 10.0)
    ResultLoad10 = "load + " + Num(Now() - TLoad) + " s:\n" + States()
    ResultLoad10Graph = Graphs("load + " + Num(Now() - TLoad) + " s")
    Debug.Trace("vibe_rm saveload Load0: " + ResultLoad0)
    Debug.Trace("vibe_rm saveload Load10: " + ResultLoad10)

    Announce(3, "Sound")
    If SkipTest.GetValue() as Int == 3
        ResultSound = "skipped by SkipTest\n"
    Else
        ResultSound = SoundAfter()
    EndIf
    Debug.Trace("vibe_rm saveload Sound: " + ResultSound)
    Game.RequestSave()
    Utility.Wait(3.0)
    Debug.MessageBox("vibe_rm saveload done. The game has saved; read the reports with vrm-tool test-results. Then, if you like, try the manual save checklist in the run instructions.")
EndEvent

; A float as text to two decimals.
String Function Num(Float f)
    String r = ""
    If f < 0.0
        r = "-"
        f = -f
    EndIf
    Int t = Math.Floor(f * 100.0 + 0.5)
    Int frac = t % 100
    r = r + (t / 100)
    If frac == 0
        return r
    EndIf
    r += "."
    If frac < 10
        r += "0"
    EndIf
    return r + frac
EndFunction

Float Function Now()
    return Utility.GetCurrentRealTime()
EndFunction

Function WaitSince(Float t0, Float s)
    While Now() - t0 < s
        Utility.Wait(0.05)
    EndWhile
EndFunction

Function Announce(Int n, String name)
    Debug.Notification("vibe_rm saveload " + n + "/3: " + name)
EndFunction

; A save that holds every report so far (a crash keeps them).
Function Checkpoint()
    Game.RequestSave()
    Utility.Wait(3.0)
EndFunction

String Function Pkg(Package p)
    If p == None
        return "none"
    ElseIf p == PkgSit
        return "Sit"
    ElseIf p == PkgSleep
        return "Sleep"
    ElseIf p == PkgSandbox
        return "Sandbox"
    ElseIf p == PkgCast
        return "Cast"
    ElseIf p == PkgWalkFar
        return "WalkFar"
    ElseIf p == PkgWalkHome
        return "WalkHome"
    ElseIf p == PkgSneakHome
        return "SneakHome"
    ElseIf p == PkgFightHome
        return "FightHome"
    ElseIf p == PkgTargetHome
        return "TargetHome"
    EndIf
    return "#" + p.GetFormID()
EndFunction

String Function B(Bool b)
    If b
        return "1"
    EndIf
    return "0"
EndFunction

; The difference of two headings in degrees, -180 to 180.
Float Function Turn(Float a, Float b)
    Float d = a - b
    While d > 180.0
        d -= 360.0
    EndWhile
    While d < -180.0
        d += 360.0
    EndWhile
    return d
EndFunction

; One actor's state, a line: sit and sleep states, sneaking, weapon drawn,
; combat state, package, 3D, and its move and turn since the save.
String Function State(Int k)
    Actor a = Actors[k]
    If a == None
        return Names[k] + ": none\n"
    EndIf
    String s = Names[k] + ": sit" + a.GetSitState() + " slp" + a.GetSleepState()
    s += " snk" + B(a.IsSneaking()) + " drw" + B(a.IsWeaponDrawn())
    s += " cmb" + a.GetCombatState() + " " + Pkg(a.GetCurrentPackage())
    s += " 3D" + B(a.Is3DLoaded())
    Float dx = a.GetPositionX() - SavedX[k]
    Float dy = a.GetPositionY() - SavedY[k]
    Float dz = a.GetPositionZ() - SavedZ[k]
    s += " moved " + Num(Math.Sqrt(dx * dx + dy * dy)) + "/" + Num(dz)
    s += " turned " + Num(Turn(a.GetAngleZ(), SavedA[k]))
    If k == 4
        s += " tgt" + B(a.GetCombatTarget() == Target)
    ElseIf k == 5
        s += " hp" + Num(a.GetActorValue("Health"))
    ElseIf k == 6
        s += " mp" + Num(a.GetActorValue("Magicka"))
    ElseIf k == 7
        s += " fromHome " + Num(a.GetDistance(WalkerHome)) + " toFar " + Num(a.GetDistance(WalkerFar))
    EndIf
    return s + "\n"
EndFunction

String Function States()
    String s = ""
    Int k = 0
    While k < 8
        s += State(k)
        k += 1
    EndWhile
    return s
EndFunction

; Graph variables the vanilla humanoid project has, a line an actor.
String Function Graph(Int k)
    Actor a = Actors[k]
    If a == None
        return Names[k] + ": none\n"
    EndIf
    String s = Names[k] + ": iState " + a.GetAnimationVariableInt("iState")
    s += " sneak " + B(a.GetAnimationVariableBool("IsSneaking"))
    s += " sync " + B(a.GetAnimationVariableBool("bIsSynced"))
    s += " animDrv " + B(a.GetAnimationVariableBool("bAnimationDriven"))
    s += " idleLoco " + a.GetAnimationVariableInt("iSyncIdleLocomotion")
    s += " 1HM " + a.GetAnimationVariableInt("i1HMState")
    s += " rHand " + a.GetAnimationVariableInt("iRightHandType")
    s += " atk " + B(a.GetAnimationVariableBool("IsAttacking"))
    s += " castR " + B(a.GetAnimationVariableBool("IsCastingRight"))
    s += " equip " + B(a.GetAnimationVariableBool("IsEquipping"))
    s += " stag " + B(a.GetAnimationVariableBool("IsStaggering"))
    s += " getUp " + a.GetAnimationVariableInt("iGetUpType")
    s += " speed " + Num(a.GetAnimationVariableFloat("Speed"))
    return s + "\n"
EndFunction

String Function Graphs(String when)
    String s = when + ":\n"
    Int k = 0
    While k < 8
        s += Graph(k)
        k += 1
    EndWhile
    return s
EndFunction

; Where every actor is now: what "moved" and "turned" are measured from.
Function Remember()
    Int k = 0
    While k < 8
        Actor a = Actors[k]
        If a != None
            SavedX[k] = a.GetPositionX()
            SavedY[k] = a.GetPositionY()
            SavedZ[k] = a.GetPositionZ()
            SavedA[k] = a.GetAngleZ()
        EndIf
        k += 1
    EndWhile
EndFunction

; The player moved to the warehouse; each actor put in its state, and up
; to 90 s for all of them to be in it.
String Function Setup()
    Go.SetValue(0.0)
    Player.MoveTo(Entry, 0.0, 0.0, 0.0, true)
    Float t0 = Now()
    Bool all = false
    While !all && Now() - t0 < 30.0
        Utility.Wait(0.5)
        all = true
        Int k = 0
        While k < 8
            If Actors[k] == None || !Actors[k].Is3DLoaded()
                all = false
            EndIf
            k += 1
        EndWhile
    EndWhile
    String r = "3D after " + Num(Now() - t0) + " s (all: " + B(all) + ")\n"

    Target.EnableAI(false)
    Target.SetActorValue("Health", 100000.0)
    Fighter.AddItem(Sword, 1, true)
    Fighter.EquipItem(Sword, false, true)
    Caster.AddSpell(Flames, false)
    Caster.SetActorValue("Magicka", 100000.0)
    Int k = 0
    While k < 8
        Actors[k].EvaluatePackage()
        k += 1
    EndWhile
    Fighter.StartCombat(Target)
    Bool sneakAgain = true

    ; Each state's first reading, -1 if not reached in 90 s.
    Float tSit = -1.0
    Float tSleep = -1.0
    Float tSneak = -1.0
    Float tFight = -1.0
    Float tCast = -1.0
    t0 = Now()
    While (tSit < 0.0 || tSleep < 0.0 || tSneak < 0.0 || tFight < 0.0 || tCast < 0.0) && Now() - t0 < 90.0
        Utility.Wait(0.25)
        Float t = Now() - t0
        If tSit < 0.0 && Sitter.GetSitState() == 3
            tSit = t
        EndIf
        If tSleep < 0.0 && Sleeper.GetSleepState() == 3
            tSleep = t
        EndIf
        If tSneak < 0.0 && Sneaker.IsSneaking()
            tSneak = t
        ElseIf tSneak < 0.0 && sneakAgain && t > 3.0
            ; The package's Always Sneak didn't do it. Only now: if
            ; StartSneaking toggles, it would stop an actor already sneaking.
            Sneaker.StartSneaking()
            sneakAgain = false
            r += "not sneaking at 3 s: StartSneaking at " + Num(t) + " s\n"
        EndIf
        If tFight < 0.0 && Fighter.GetCombatState() == 1
            tFight = t
        EndIf
        If tCast < 0.0 && Caster.GetAnimationVariableBool("IsCastingRight")
            tCast = t
        EndIf
        If Target.GetActorValue("Health") < 50000.0
            Target.RestoreActorValue("Health", 50000.0)
        EndIf
    EndWhile
    r += "seated " + Num(tSit) + ", asleep " + Num(tSleep) + ", sneaking " + Num(tSneak)
    r += ", fighting " + Num(tFight) + ", casting " + Num(tCast) + " s (-1: not in 90 s)\n"
    ; The sandboxer's idles and the caster's next cast get a moment more.
    Utility.Wait(5.0)
    Remember()
    r += States()
    return r
EndFunction

; The walker sent off and caught halfway, the loop started, the hearing
; question asked; then everyone read.
String Function Before()
    String r = ""
    Float whole = WalkerHome.GetDistance(WalkerFar)
    Go.SetValue(1.0)
    Walker.EvaluatePackage()
    Float t0 = Now()
    Bool half = false
    While !half && Now() - t0 < 60.0
        Utility.Wait(0.1)
        half = Walker.GetDistance(WalkerHome) >= whole * 0.5
    EndWhile
    r += "walker " + Num(Walker.GetDistance(WalkerHome)) + " of " + Num(whole) + " from home after " + Num(Now() - t0) + " s (halfway: " + B(half) + ")\n"
    LoopId = Loop.Play(Player)
    r += "loop instance " + LoopId + "\n"
    r += States()
    return r
EndFunction

; The hearing question, the save, and the user's load of it. The script
; goes on in the loaded game from the wait after the save request: a gap
; in real time (or real time going back, after a restart) means a load.
String Function SaveAndLoad()
    String r = ""
    Int heard = AskHear.Show(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
    r += "loop heard before the save: " + HearAnswer(heard) + "\n"
    ; The box paused the game; this wait runs once it's closed.
    Utility.Wait(0.5)
    Remember()
    TSave = Now()
    Game.RequestSave()
    Float last = Now()
    Float gap = 0.0
    While !Loaded && Now() - TSave < 180.0
        Utility.Wait(0.05)
        Float t = Now()
        If t - last > 5.0 || t < last
            Loaded = true
            gap = t - last
        EndIf
        last = t
    EndWhile
    TLoad = Now()
    If Loaded
        r += "resumed after a load: real time jumped " + Num(gap) + " s (negative: the game was restarted)\n"
    Else
        r += "NOT LOADED: no load within 180 s; the readings below are of the live game\n"
    EndIf
    return r
EndFunction

String Function HearAnswer(Int i)
    If i == 0
        return "yes"
    ElseIf i == 1
        return "no"
    EndIf
    return "not sure"
EndFunction

; After the load: the user asked whether the loop played on, then the
; saved instance id stopped and the user asked again.
String Function SoundAfter()
    String r = "instance " + LoopId + "\n"
    Int heard = AskHeardAfter.Show(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
    r += "loop heard after the load: " + HearAnswer(heard) + "\n"
    Utility.Wait(0.5)
    Sound.StopInstance(LoopId)
    Utility.Wait(2.0)
    Int still = AskStopped.Show(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
    If still == 0
        r += "after StopInstance: still playing\n"
    ElseIf still == 1
        r += "after StopInstance: stopped\n"
    Else
        r += "after StopInstance: wasn't playing\n"
    EndIf
    return r
EndFunction
