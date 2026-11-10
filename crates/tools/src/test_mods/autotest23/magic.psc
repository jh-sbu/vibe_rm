; Story events, magic, Disarm and detection, in the test cell.

; The Story Manager log the story quests write to (VrmAt21StoryLog).
String StoryLog = ""

Function AddStory(String s)
    StoryLog = StoryLog + s + "@" + Math.Floor(Utility.GetCurrentRealTime() * 100.0) + "; "
EndFunction

String Function TakeStory()
    Utility.Wait(4.0)
    String s = StoryLog
    StoryLog = ""
    If s == ""
        return "(none)"
    EndIf
    return s
EndFunction

; story-manager: is KILL sent for an essential actor's Kill (autotest22's
; one KILL with no killer came when an essential NPC went into bleedout)?
String Function KillCase(ActorBase b, Actor killer, String label)
    Actor a = Spawn(b, 0.0, 300.0)
    a.SetDontMove(true)
    Utility.Wait(1.0)
    (a as VrmAt21ActorLog).Take()
    a.Kill(killer)
    String r = label + " (" + Id(a) + "): story " + TakeStory()
    r += "; bleeding out " + a.IsBleedingOut() + ", dead " + a.IsDead() + ", events " + (a as VrmAt21ActorLog).Take() + "\n"
    Gone(a)
    return r
EndFunction

String Function TestEssentialKill()
    String r = "(player " + Id(Player) + ") left from earlier tests: " + TakeStory() + "\n"
    r += KillCase(EssentialLoggedBase, None, "an essential NPC, Kill(None)")
    r += KillCase(EssentialLoggedBase, Player, "an essential NPC, Kill(player)")
    r += KillCase(LoggedBase, None, "a plain NPC, Kill(None)")
    return r
EndFunction

; magic: lasting effects on an actor who dies (Invisibility's actor value,
; a cloak by its keyword).
String Function TestEffectsOnDead()
    Actor n = Spawn(NakedBase, 0.0, 300.0)
    n.SetDontMove(true)
    Utility.Wait(1.0)
    InvisibilitySpell.Cast(n, n)
    FlameCloakSpell.Cast(n, n)
    Utility.Wait(1.5)
    String r = "Invisibility and a flame cloak on an NPC: Invisibility " + Num(n.GetActorValue("Invisibility")) + ", cloak " + n.HasMagicEffectWithKeyword(MagicCloakKw)
    n.Kill(None)
    Utility.Wait(1.0)
    r += "; killed, +1 s: " + Num(n.GetActorValue("Invisibility")) + ", " + n.HasMagicEffectWithKeyword(MagicCloakKw)
    Utility.Wait(3.0)
    r += "; +4 s: " + Num(n.GetActorValue("Invisibility")) + ", " + n.HasMagicEffectWithKeyword(MagicCloakKw) + "\n"
    Gone(n)
    return r
EndFunction

; magic: does a fire-and-forget effect whose condition fails land? Our fire
; spell (10 points) with IsWin32 == 0 (IsWin32 holds in SE), then without.
String Function TestFailingCondition()
    Actor n = Spawn(LoggedBase, 0.0, 300.0)
    n.SetDontMove(true)
    n.ForceActorValue("HealRate", 0.0)
    Utility.Wait(1.0)
    (n as VrmAt21ActorLog).Take()
    Float h0 = n.GetActorValue("Health")
    FailingFireSpell.Cast(Player, n)
    Utility.Wait(2.0)
    Float h1 = n.GetActorValue("Health")
    String r = "fire 10 whose condition fails: health " + Num(h0) + " -> " + Num(h1) + ", events " + (n as VrmAt21ActorLog).Take()
    WhFireSpell.Cast(Player, n)
    Utility.Wait(2.0)
    r += "; the same with no condition: -> " + Num(n.GetActorValue("Health")) + ", events " + (n as VrmAt21ActorLog).Take() + "\n"
    n.StopCombat()
    Gone(n)
    return r
EndFunction

; magic: Slow Time's share of the world and of the game clock, sampled
; through and after the effect, for each word (autotest21: the fall said
; 0.5, the clock 0.35, and the fall stayed slow after the effect).
Float Function Fall()
    ObjectReference b = CellCenter.PlaceAtMe(Bowl, 1, false, false)
    b.MoveTo(CellCenter, 200.0, 200.0, 500.0, false)
    Utility.Wait(0.05)
    b.SetMotionType(1, true)
    Float z = b.GetPositionZ()
    Float t0 = Now()
    Utility.Wait(0.4)
    Float fell = z - b.GetPositionZ()
    Float dt = Now() - t0
    Gone(b)
    return fell / dt
EndFunction

; The game clock's speed over secs real seconds, as a share of the
; timescale's.
Float Function Clock(Float secs)
    Float g0 = GameHour.GetValue()
    Float t0 = Now()
    Utility.Wait(secs)
    Float dg = GameHour.GetValue() - g0
    If dg < 0.0
        dg += 24.0
    EndIf
    return dg * 3600.0 / (TimeScale.GetValue() * (Now() - t0))
EndFunction

String Function SlowWord(Spell s, Float length, String label)
    String r = label + ": before, fall " + Num(Fall()) + ", clock " + Num(Clock(1.0)) + "; t (real s): on, fall, clock:"
    Float t0 = Now()
    s.Cast(Player, Player)
    While Now() - t0 < length + 8.0
        Bool on = Player.HasMagicEffect(SlowTimeEffect)
        Float t = Now() - t0
        Float f = Fall()
        r += " " + Num(t) + ":" + on + "," + Num(f) + "," + Num(Clock(0.5))
    EndWhile
    Player.DispelSpell(s)
    Utility.Wait(3.0)
    return r + "\n"
EndFunction

String Function TestSlowTime()
    String r = SlowWord(SlowTimeSpell, 8.0, "word 1 (0.3, 8 s)")
    r += SlowWord(SlowTime2Spell, 12.0, "word 2 (0.2, 12 s)")
    r += SlowWord(SlowTime3Spell, 16.0, "word 3 (0.1, 16 s)")
    return r
EndFunction

; disarm: the shout's spell cast from close by, and a target-actor copy of
; its effect at magnitude 12 and 30, on NPCs of levels 1, 10 and 20 fighting
; a held target; a dagger in each one's pack to see what it fights with.
String Function DisarmCase(ActorBase b, Form weapon, Spell s, Bool near, Bool logged, String label)
    Actor a = Spawn(b, 0.0, 250.0)
    Actor t = Spawn(InvulnerableBase, 0.0, 450.0)
    t.SetDontMove(true)
    a.AddItem(weapon, 1, true)
    a.AddItem(Dagger, 1, true)
    a.EquipItem(weapon, false, true)
    Utility.Wait(1.0)
    a.StartCombat(t)
    Utility.Wait(3.0)
    If logged
        (a as VrmAt21ActorLog).Take()
    EndIf
    String r = label + " (level " + a.GetLevel() + "): drawn " + a.IsWeaponDrawn() + ", combat " + a.GetCombatState() + ", holds " + Id(a.GetEquippedWeapon(false))
    If near
        Player.MoveTo(a, 0.0, -150.0, 0.0, false)
        Utility.Wait(0.5)
    EndIf
    s.Cast(Player, a)
    Utility.Wait(2.0)
    r += "; after: holds " + Id(a.GetEquippedWeapon(false)) + ", carries " + a.GetItemCount(weapon)
    If logged
        r += ", events " + (a as VrmAt21ActorLog).Take()
    EndIf
    ObjectReference w = Game.FindClosestReferenceOfTypeFromRef(weapon, a, 2000.0)
    If w != None && w.GetParentCell() == a.GetParentCell()
        r += "; on the floor: dx " + Num(w.GetPositionX() - a.GetPositionX()) + ", dy " + Num(w.GetPositionY() - a.GetPositionY()) + ", dz " + Num(w.GetPositionZ() - a.GetPositionZ())
    EndIf
    Utility.Wait(4.0)
    r += "; 4 s later: holds " + Id(a.GetEquippedWeapon(false)) + ", drawn " + a.IsWeaponDrawn() + ", combat " + a.GetCombatState()
    If w != None
        r += ", the dropped one " + Num(a.GetDistance(w)) + " off, carried " + a.GetItemCount(weapon)
    EndIf
    a.StopCombat()
    Gone(a)
    Gone(t)
    Gone(w)
    If near
        GoCell()
    EndIf
    return r + "\n"
EndFunction

String Function TestDisarm()
    String r = "(iron sword " + Id(IronSword) + ", greatsword " + Id(IronGreatsword) + ", dagger " + Id(Dagger) + ")\n"
    r += DisarmCase(LowLoggedBase, IronSword, DisarmSpell, true, true, "the shout's spell from 150 units")
    r += DisarmCase(LowLoggedBase, IronSword, OurDisarm, false, true, "our Disarm 12")
    r += DisarmCase(LoggedBase, IronSword, OurDisarm, false, true, "our Disarm 12")
    r += DisarmCase(Level20FighterBase, IronGreatsword, OurDisarm, false, false, "our Disarm 12, a greatsword")
    r += DisarmCase(Level20FighterBase, IronGreatsword, OurDisarmStrong, false, false, "our Disarm 30, a greatsword")
    return r
EndFunction

; detection: the view cone's edge between 90 (seen in autotest22) and 135
; (not), with the skills behind it.
String Function SeenAt(Actor o, Float x, Float y)
    Player.MoveTo(o, x, y, 0.0, false)
    Utility.Wait(2.0)
    String r = "" + Player.IsDetectedBy(o)
    Utility.Wait(3.0)
    return r + "/" + Player.IsDetectedBy(o)
EndFunction

String Function TestViewCone()
    Actor o = Spawn(NakedBase, 0.0, 0.0)
    o.SetDontMove(true)
    o.MoveTo(CellCenter, -1300.0, -1300.0, 0.0, false)
    o.SetAngle(0.0, 0.0, 45.0)
    Utility.Wait(1.0)
    Player.StartSneaking()
    Utility.Wait(1.0)
    String r = "observer (level " + o.GetLevel() + ", Sneak " + Num(o.GetActorValue("Sneak")) + ") facing north-east; the player sneaking " + Player.IsSneaking() + ", Sneak " + Num(Player.GetActorValue("Sneak")) + ", level " + Player.GetLevel() + ", light " + Num(Player.GetLightLevel()) + "; at 300, by bearing from its facing, seen at 2 s/5 s:"
    Int deg = 90
    While deg <= 135
        Float a = 45.0 + deg
        r += " " + deg + ":" + SeenAt(o, 300.0 * Math.sin(a), 300.0 * Math.cos(a))
        deg += 9
    EndWhile
    Gone(o)
    GoCell()
    If Player.IsSneaking()
        Player.StartSneaking()
    EndIf
    return r + "\n"
EndFunction

; alias-fills: create levels, several picks each, at the save's player level
; (autotest22's level 1 couldn't separate them).
String Function TestCreateLevel()
    String r = "player level " + Player.GetLevel()
    If Player.GetLevel() < 5
        return r + ": skipped (needs a player of level 5 or more)\n"
    EndIf
    Int i = 0
    While i < 2
        Quest q = LevelQuests.GetAt(i) as Quest
        r += "\n  create level " + (i * 3) + ":"
        Int k = 0
        While k < 5
            q.Start()
            Utility.Wait(1.0)
            Actor made = (q.GetAlias(1) as ReferenceAlias).GetActorReference()
            If made == None
                r += " none"
            Else
                r += " " + made.GetLevel() + "/" + Id(made.GetLeveledActorBase())
            EndIf
            q.Stop()
            Utility.Wait(0.5)
            Gone(made)
            k += 1
        EndWhile
        i += 1
    EndWhile
    return r + "\n"
EndFunction
