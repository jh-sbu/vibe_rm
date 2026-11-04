Scriptname VrmRegenTestScript extends Quest
; test-mod regen: how the player's health, magicka and stamina come back:
; the wait after damage, the rate multipliers, and combat. regen.rs
; documents the readings each reading predicts.

Actor Property Player Auto
ActorBase Property EnemyBase Auto
Spell Property Bolt Auto

Bool Ran = false
String[] AVs
String[] Mults
Float[] Maxes
Actor Enemy
Float RiseAt
Float RiseAfter

String ResultSetup
String ResultDelay
String ResultRepeat
String ResultCross
String ResultRates
String ResultCombatRates
String ResultHit

Event OnInit()
    RegisterForSingleUpdate(5.0)
EndEvent

Event OnUpdate()
    If Ran
        return
    EndIf
    Ran = true
    Debug.MessageBox("vibe_rm regen: about 3 minutes of automatic readings of your health, magicka and stamina. Stand still and touch nothing until the done box. A mudcrab appears for the combat part; it can't move or fight.")
    Utility.Wait(1.0)
    ResultSetup = Setup()
    Debug.Trace("vibe_rm regen " + ResultSetup)
    ResultDelay = TestDelay()
    Debug.Trace("vibe_rm regen " + ResultDelay)
    ResultRepeat = TestRepeat()
    Debug.Trace("vibe_rm regen " + ResultRepeat)
    ResultCross = TestCross()
    Debug.Trace("vibe_rm regen " + ResultCross)
    ResultRates = TestRates()
    Debug.Trace("vibe_rm regen " + ResultRates)
    ResultCombatRates = TestCombatRates()
    Debug.Trace("vibe_rm regen " + ResultCombatRates)
    ResultHit = TestHit()
    Debug.Trace("vibe_rm regen " + ResultHit)
    Finish()
    Game.RequestSave()
    Utility.Wait(3.0)
    Debug.MessageBox("vibe_rm regen done. Read the reports with vrm-tool test-results.")
EndEvent

; f to 3 decimals.
String Function Num(Float f)
    String r = ""
    If f < 0.0
        r = "-"
        f = -f
    EndIf
    Int t = Math.Floor(f * 1000.0 + 0.5)
    Int frac = t % 1000
    r = r + (t / 1000) + "."
    If frac < 100
        r += "0"
    EndIf
    If frac < 10
        r += "0"
    EndIf
    return r + frac
EndFunction

Float Function Now()
    return Utility.GetCurrentRealTime()
EndFunction

; Waits until s seconds after t0 (at once if that has passed).
Function WaitUntil(Float t0, Float s)
    Float left = s - (Now() - t0)
    If left > 0.0
        Utility.Wait(left)
    EndIf
EndFunction

Float Function Value(Int i)
    return Player.GetActorValue(AVs[i])
EndFunction

; All three back to full.
Function Fill()
    Int i = 0
    While i < 3
        Player.RestoreActorValue(AVs[i], 100000.0)
        i += 1
    EndWhile
EndFunction

; Value i damaged down to a fraction of its most.
Function DownTo(Int i, Float frac)
    Float by = Value(i) - frac * Maxes[i]
    If by > 0.0
        Player.DamageActorValue(AVs[i], by)
    EndIf
EndFunction

String Function Reading(String av)
    return av + " " + Num(Player.GetActorValue(av)) + " (base " + Num(Player.GetBaseActorValue(av)) + ")"
EndFunction

String Function Setting(String name)
    return name + " " + Num(Game.GetGameSettingFloat(name))
EndFunction

String Function Setup()
    AVs = new String[3]
    AVs[0] = "Health"
    AVs[1] = "Magicka"
    AVs[2] = "Stamina"
    Mults = new String[3]
    Mults[0] = "HealRateMult"
    Mults[1] = "MagickaRateMult"
    Mults[2] = "StaminaRateMult"
    Maxes = new Float[3]
    Fill()
    Utility.Wait(0.5)
    String r = "Setup: in combat " + Player.IsInCombat() + "; full"
    Int i = 0
    While i < 3
        Maxes[i] = Value(i)
        r += " " + AVs[i] + " " + Num(Maxes[i])
        i += 1
    EndWhile
    r += "\n  " + Reading("HealRate") + ", " + Reading("MagickaRate") + ", " + Reading("StaminaRate")
    r += "\n  " + Reading("HealRateMult") + ", " + Reading("MagickaRateMult") + ", " + Reading("StaminaRateMult")
    r += "\n  " + Reading("CombatHealthRegenMult")
    r += "\n  " + Setting("fDamagedHealthRegenDelay") + ", " + Setting("fDamagedMagickaRegenDelay") + ", " + Setting("fDamagedStaminaRegenDelay") + ", " + Setting("fDamagedAVRegenDelay")
    r += "\n  " + Setting("fCombatHealthRegenRateMult") + ", " + Setting("fCombatMagickaRegenRateMult") + ", " + Setting("fCombatStaminaRegenRateMult") + "\n"
    return r
EndFunction

; Samples value i from t0 (when it stood at v0) every frame or so until it
; rises past v0, or timeout seconds: RiseAt the first sample above v0
; (-1 if none), RiseAfter the last sample before it.
Function Watch(Int i, Float t0, Float v0, Float timeout)
    RiseAt = -1.0
    RiseAfter = 0.0
    Float t = Now() - t0
    While t < timeout
        If Value(i) > v0 + 0.0005
            RiseAt = t
            return
        EndIf
        RiseAfter = t
        Utility.Wait(0.02)
        t = Now() - t0
    EndWhile
EndFunction

String Function Rise()
    If RiseAt < 0.0
        return "no rise"
    EndIf
    return "rose between " + Num(RiseAfter) + " and " + Num(RiseAt) + " s"
EndFunction

; One drop to half by DamageActorValue: when does the value first rise,
; and how much is back 1 and 2 s after the drop?
String Function TestDelay()
    String r = "Delay: each value full, then damaged to half (DamageActorValue):\n"
    Int i = 0
    While i < 3
        Fill()
        Utility.Wait(1.5)
        DownTo(i, 0.5)
        Float t0 = Now()
        Float v0 = Value(i)
        Watch(i, t0, v0, 3.0)
        r += "  " + AVs[i] + ": " + Rise()
        WaitUntil(t0, 1.0)
        r += "; back by 1 s " + Num(Value(i) - v0)
        WaitUntil(t0, 2.0)
        r += ", by 2 s " + Num(Value(i) - v0) + "\n"
        i += 1
    EndWhile
    return r
EndFunction

; Five more hits of 2% every 0.3 s after the first drop (shorter than the
; 0.5 s setting): does each start the wait again?
String Function TestRepeat()
    String r = "Repeat: damaged to half, then 2% more every 0.3 s five times:\n"
    Int i = 0
    While i < 3
        Fill()
        Utility.Wait(1.5)
        DownTo(i, 0.5)
        Float first = Now()
        String between = ""
        Int hit = 0
        While hit < 5
            Float t0 = Now()
            Float v0 = Value(i)
            Watch(i, t0, v0, 0.3)
            If RiseAt >= 0.0
                between += " " + hit + ":" + Num(RiseAt)
            EndIf
            WaitUntil(t0, 0.3)
            Player.DamageActorValue(AVs[i], 0.02 * Maxes[i])
            hit += 1
        EndWhile
        Float t1 = Now()
        Watch(i, t1, Value(i), 3.0)
        r += "  " + AVs[i] + ": hits over " + Num(t1 - first) + " s; rises between hits"
        If between == ""
            between = " none"
        EndIf
        r += between + "; after the last " + Rise() + "\n"
        i += 1
    EndWhile
    return r
EndFunction

; The other two values' gain in the 0.5 s before and after one is damaged:
; is the wait shared?
String Function Gains(Int hurt, Float[] before, Float[] mid)
    String r = ""
    Int i = 0
    While i < 3
        If i != hurt
            r += " " + AVs[i] + " " + Num(mid[i] - before[i]) + " then " + Num(Value(i) - mid[i])
        EndIf
        i += 1
    EndWhile
    return r
EndFunction

String Function TestCross()
    String r = "Cross: all at 40%, regenerating; one damaged 20% more; the others' gain 0.5 s before / after:\n"
    Int hurt = 0
    While hurt < 3
        Fill()
        Utility.Wait(1.0)
        Int i = 0
        While i < 3
            DownTo(i, 0.4)
            i += 1
        EndWhile
        Utility.Wait(1.5)
        Float[] before = new Float[3]
        Float[] mid = new Float[3]
        i = 0
        While i < 3
            before[i] = Value(i)
            i += 1
        EndWhile
        Utility.Wait(0.5)
        i = 0
        While i < 3
            mid[i] = Value(i)
            i += 1
        EndWhile
        Player.DamageActorValue(AVs[hurt], 0.2 * Maxes[hurt])
        Utility.Wait(0.5)
        r += "  " + AVs[hurt] + " hit:" + Gains(hurt, before, mid) + "\n"
        hurt += 1
    EndWhile
    return r
EndFunction

; Value i's rate, percent of its most a second, measured from 1.5 s after a
; drop to low for span seconds.
Float Function Rate(Int i, Float low, Float span)
    Fill()
    Utility.Wait(1.0)
    DownTo(i, low)
    Utility.Wait(1.5)
    Float t0 = Now()
    Float v0 = Value(i)
    Utility.Wait(span)
    return (Value(i) - v0) / Maxes[i] * 100.0 / (Now() - t0)
EndFunction

; Each value's rate as it is, with its rate multiplier +100 and, for
; health, CombatHealthRegenMult halved (-0.35).
String Function Rates()
    String r = ""
    Int i = 0
    While i < 3
        Float low = 0.1
        Float span = 3.0
        If i == 0
            low = 0.4
            span = 5.0
        EndIf
        r += "  " + AVs[i] + " %/s: " + Num(Rate(i, low, span))
        Player.ModActorValue(Mults[i], 100.0)
        r += "; " + Mults[i] + " " + Num(Player.GetActorValue(Mults[i])) + ": " + Num(Rate(i, low, span))
        Player.ModActorValue(Mults[i], -100.0)
        If i == 0
            Player.ModActorValue("CombatHealthRegenMult", -0.35)
            r += "; CombatHealthRegenMult " + Num(Player.GetActorValue("CombatHealthRegenMult")) + ": " + Num(Rate(i, low, span))
            Player.ModActorValue("CombatHealthRegenMult", 0.35)
        EndIf
        r += "\n"
        i += 1
    EndWhile
    return r
EndFunction

String Function TestRates()
    return "Rates out of combat (in combat " + Player.IsInCombat() + "):\n" + Rates() + "  still out of combat: " + !Player.IsInCombat() + "\n"
EndFunction

; A mudcrab 600 units off, AI off, fighting the player.
String Function TestCombatRates()
    Enemy = Player.PlaceAtMe(EnemyBase, 1, false, false) as Actor
    Enemy.MoveTo(Player, 600.0, 0.0, 0.0, false)
    Utility.Wait(1.0)
    Enemy.EnableAI(false)
    Enemy.StartCombat(Player)
    Utility.Wait(2.0)
    String r = "Rates in combat (mudcrab, AI off, StartCombat(player): its state " + Enemy.GetCombatState() + ", player in combat " + Player.IsInCombat() + "):\n"
    r += Rates()
    r += "  after: its state " + Enemy.GetCombatState() + ", player in combat " + Player.IsInCombat() + "\n"
    return r
EndFunction

; A real hit: Firebolt from the mudcrab. When does health rise after it
; lands, and do magicka and stamina pause?
String Function TestHit()
    Fill()
    Utility.Wait(1.0)
    DownTo(0, 0.8)
    DownTo(1, 0.4)
    DownTo(2, 0.4)
    Utility.Wait(1.5)
    Float h = Value(0)
    Float m = Value(1)
    Float s = Value(2)
    Bolt.Cast(Enemy, Player)
    Float cast = Now()
    While Value(0) >= h && Now() - cast < 5.0
        h = Value(0)
        m = Value(1)
        s = Value(2)
        Utility.Wait(0.02)
    EndWhile
    Float t0 = Now()
    Float v0 = Value(0)
    String r = "Hit (Firebolt from the mudcrab, player in combat " + Player.IsInCombat() + "):\n"
    If v0 >= h
        return r + "  never landed in 5 s\n"
    EndIf
    r += "  landed " + Num(t0 - cast) + " s after the cast, health " + Num(h - v0) + " lost; magicka, stamina gained " + Num(Value(1) - m) + " " + Num(Value(2) - s) + " at the landing\n"
    Float m0 = Value(1)
    Float s0 = Value(2)
    ; Watch keeps time from t0, so the second call carries on from 0.5 s.
    Watch(0, t0, v0, 0.5)
    WaitUntil(t0, 0.5)
    String gains = Num(Value(1) - m0) + " " + Num(Value(2) - s0)
    If RiseAt < 0.0
        Watch(0, t0, v0, 3.0)
    EndIf
    r += "  health " + Rise() + "; magicka, stamina in the first 0.5 s " + gains + "\n"
    return r
EndFunction

Function Finish()
    If Enemy != None
        Enemy.EnableAI(true)
        Enemy.StopCombat()
        Player.StopCombatAlarm()
        Utility.Wait(0.5)
        Enemy.Disable(false)
        Enemy.Delete()
    EndIf
    Fill()
EndFunction
