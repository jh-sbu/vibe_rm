; Fights in WarehouseFurniture. The player gets 1000 extra health here and
; loses it on leaving.

String Function TestEnterWarehouse()
    GoWarehouse()
    Player.ModActorValue("Health", 1000.0)
    Player.RestoreActorValue("Health", 1000.0)
    return "in the warehouse: interior " + Player.IsInInterior() + ", WhCenter distance " + Num(Player.GetDistance(WhCenter)) + ", location " + Id(Player.GetCurrentLocation()) + "\n"
EndFunction

; staffs: an NPC's enchanted blows, the charge and the target's health
; every half second (each hit shows in the health), at Enchanting 15 and
; 100 (autotest21: about 1.9 a blow, ours about 3.6).
String Function BlowCase(Float enchanting)
    Actor a = SpawnWh(RedFighterBase, 0.0, 0.0)
    Actor t = SpawnWh(LoggedBase, 150.0, 0.0)
    t.SetDontMove(true)
    t.ForceActorValue("Health", 5000.0)
    t.ForceActorValue("HealRate", 0.0)
    a.ForceActorValue("Health", 5000.0)
    a.ForceActorValue("Enchanting", enchanting)
    a.AddItem(FireSword, 1, true)
    a.EquipItem(FireSword, false, true)
    Utility.Wait(1.5)
    (t as VrmAt21ActorLog).Take()
    String r = "OneHanded " + Num(a.GetActorValue("OneHanded")) + ", Enchanting " + Num(a.GetActorValue("Enchanting")) + ", holds " + Id(a.GetEquippedWeapon(false)) + "; charge/target health by 0.5 s:"
    a.StartCombat(t)
    Int k = 0
    While k < 24
        Utility.Wait(0.5)
        r += " " + Num(a.GetActorValue("RightItemCharge")) + "/" + Num(t.GetActorValue("Health"))
        k += 1
    EndWhile
    r += "\n  the target's events " + (t as VrmAt21ActorLog).Take() + "\n"
    a.StopCombat()
    t.StopCombat()
    Gone(a)
    Gone(t)
    return r
EndFunction

String Function TestEnchantedBlows()
    return "(sword of burning " + Id(FireSword) + ")\n" + BlowCase(15.0) + BlowCase(100.0)
EndFunction

; staffs: an NPC's staffs. Which hand, the empty hand's type, the charge
; every second at Destruction as given, 100, and DestructionMod 50; a
; fire-and-forget staff; and a staff as the only weapon carried.
String Function StaffCase(Form staff, Float destruction, Float mod, Bool stripped, String label)
    Actor s = SpawnWh(StaffWarlockBase, 0.0, 0.0)
    Actor t = SpawnWh(InvulnerableBase, 500.0, 0.0)
    t.SetDontMove(true)
    t.ForceActorValue("Health", 2000.0)
    Utility.Wait(1.0)
    If stripped
        s.RemoveAllItems(None, false, false)
    EndIf
    If destruction >= 0.0
        s.ForceActorValue("Destruction", destruction)
    EndIf
    s.ForceActorValue("DestructionMod", mod)
    s.AddItem(staff, 1, true)
    s.EquipItem(staff, false, true)
    Utility.Wait(1.5)
    (s as VrmAt21ActorLog).Take()
    String r = label + " (Destruction " + Num(s.GetActorValue("Destruction")) + ", DestructionMod " + Num(s.GetActorValue("DestructionMod")) + "): right " + Id(s.GetEquippedWeapon(false)) + " type " + s.GetEquippedItemType(1) + ", left " + Id(s.GetEquippedWeapon(true)) + " type " + s.GetEquippedItemType(0) + "; charge right/left by 1 s:"
    s.StartCombat(t)
    Int k = 0
    While k < 10
        Utility.Wait(1.0)
        r += " " + Num(s.GetActorValue("RightItemCharge")) + "/" + Num(s.GetActorValue("LeftItemCharge"))
        k += 1
    EndWhile
    r += "; then right type " + s.GetEquippedItemType(1) + ", left " + s.GetEquippedItemType(0) + "; events " + (s as VrmAt21ActorLog).Take() + "\n"
    s.StopCombat()
    Gone(s)
    Gone(t)
    return r
EndFunction

String Function TestNpcStaffs()
    String r = "(sparks " + Id(SparksStaff) + ", firebolt " + Id(FireboltStaff) + ")\n"
    r += StaffCase(SparksStaff, -1.0, 0.0, false, "sparks")
    r += StaffCase(SparksStaff, 100.0, 0.0, false, "sparks, Destruction 100")
    r += StaffCase(SparksStaff, -1.0, 50.0, false, "sparks, DestructionMod 50")
    r += StaffCase(FireboltStaff, -1.0, 0.0, false, "firebolt")
    r += StaffCase(SparksStaff, -1.0, 0.0, true, "sparks, nothing else carried")
    return r
EndFunction

; magic: Fear, Calm and Fury read by IsFleeing, the actor values they hold
; and the effect events (autotest22's distances showed nothing).
String Function TestHolds()
    String r = ""
    Float[] res = new Float[2]
    res[0] = 0.0
    res[1] = 90.0
    Int i = 0
    While i < 2
        Actor b = SpawnWh(Level8LoggedBase, 0.0, 0.0)
        Actor t = SpawnWh(InvulnerableBase, 200.0, 0.0)
        t.SetDontMove(true)
        b.ForceActorValue("MagicResist", res[i])
        b.AddItem(IronSword, 1, true)
        b.EquipItem(IronSword, false, true)
        Utility.Wait(1.0)
        b.StartCombat(t)
        Utility.Wait(3.0)
        Float d0 = b.GetDistance(t)
        String c0 = Num(b.GetActorValue("Confidence"))
        (b as VrmAt21ActorLog).Take()
        FearSpell.Cast(Player, b)
        Utility.Wait(1.0)
        r += "Fear on a level 8 NPC, magic resistance " + Num(res[i]) + ": confidence " + c0 + " -> " + Num(b.GetActorValue("Confidence")) + ", IsFleeing " + Probe(b, PS_Fleeing, PE_Fleeing)
        r += ", distance " + Num(d0) + " -> " + Num(b.GetDistance(t)) + ", combat " + b.GetCombatState() + ", events " + (b as VrmAt21ActorLog).Take() + "\n"
        b.StopCombat()
        Gone(b)
        Gone(t)
        i += 1
    EndWhile
    Actor c = SpawnWh(LowFighterLoggedBase, 0.0, 0.0)
    Actor ct = SpawnWh(InvulnerableBase, 200.0, 0.0)
    ct.SetDontMove(true)
    Utility.Wait(1.0)
    c.StartCombat(ct)
    Utility.Wait(3.0)
    (c as VrmAt21ActorLog).Take()
    r += "Calm on a level 1 fighter (combat " + c.GetCombatState() + ", aggression " + Num(c.GetActorValue("Aggression")) + "): combat by second"
    CalmSpell.Cast(Player, c)
    Int s = 0
    While s < 4
        Utility.Wait(1.0)
        r += " " + c.GetCombatState()
        s += 1
    EndWhile
    r += ", aggression " + Num(c.GetActorValue("Aggression")) + ", events " + (c as VrmAt21ActorLog).Take() + "\n"
    c.StopCombat()
    Gone(c)
    Gone(ct)
    Actor f = SpawnWh(LowLoggedBase, 0.0, 0.0)
    Actor n = SpawnWh(NakedBase, 200.0, 0.0)
    n.SetDontMove(true)
    Utility.Wait(1.0)
    (f as VrmAt21ActorLog).Take()
    r += "Fury on a level 1 unaggressive NPC (aggression " + Num(f.GetActorValue("Aggression")) + "), another 200 off: combat/target by second"
    FurySpell.Cast(Player, f)
    s = 0
    While s < 5
        Utility.Wait(1.0)
        String who = "none"
        If f.GetCombatTarget() == n
            who = "other"
        ElseIf f.GetCombatTarget() == Player
            who = "player"
        EndIf
        r += " " + f.GetCombatState() + "/" + who
        s += 1
    EndWhile
    r += ", aggression " + Num(f.GetActorValue("Aggression")) + ", confidence " + Num(f.GetActorValue("Confidence")) + ", events " + (f as VrmAt21ActorLog).Take() + "\n"
    f.StopCombat()
    n.StopCombat()
    Gone(f)
    Gone(n)
    Player.RestoreActorValue("Health", 2000.0)
    return r
EndFunction

; npc-healing: the Vigilant hurt in a fight, with magicka enough for its
; heal (autotest22's had 21 to 43).
String Function TestHealer()
    Actor h = SpawnWh(VigilantLogBase, 0.0, 0.0)
    Actor t = SpawnWh(InvulnerableBase, 200.0, 0.0)
    t.SetDontMove(true)
    h.ForceActorValue("HealRate", 0.0)
    h.SetActorValue("Magicka", 300.0)
    h.RestoreActorValue("Magicka", 1000.0)
    Utility.Wait(2.0)
    (h as VrmAt21ActorLog).Take()
    h.StartCombat(t)
    Utility.Wait(2.0)
    String r = "a Vigilant (level " + h.GetLevel() + ", Restoration " + Num(h.GetActorValue("Restoration")) + ", magicka " + Num(h.GetActorValue("Magicka")) + ", spells " + Id(h.GetEquippedSpell(0)) + " / " + Id(h.GetEquippedSpell(1)) + ") fighting, by health fraction:"
    Float[] at = new Float[3]
    at[0] = 0.7
    at[1] = 0.45
    at[2] = 0.2
    Int i = 0
    While i < 3
        h.RestoreActorValue("Health", 10000.0)
        h.RestoreActorValue("Magicka", 1000.0)
        h.DamageActorValue("Health", h.GetActorValueMax("Health") * (1.0 - at[i]))
        Float before = h.GetActorValuePercentage("Health")
        Float m = h.GetActorValue("Magicka")
        Utility.Wait(8.0)
        r += "\n  at " + Num(before) + " -> " + Num(h.GetActorValuePercentage("Health")) + ", magicka " + Num(m) + " -> " + Num(h.GetActorValue("Magicka")) + " (" + (h as VrmAt21ActorLog).Take() + ")"
        i += 1
    EndWhile
    h.StopCombat()
    Gone(h)
    Gone(t)
    return r + "\n"
EndFunction

; wards: the Vigilant with magicka enough, held 700 from a fire warlock.
String Function TestWards()
    Actor v = SpawnWh(VigilantLogBase, 0.0, 0.0)
    Actor w = SpawnWh(WarlockLogBase, 700.0, 0.0)
    v.SetDontMove(true)
    w.SetDontMove(true)
    v.ForceActorValue("HealRate", 0.0)
    v.ForceActorValue("Health", 2000.0)
    v.SetActorValue("Magicka", 300.0)
    v.RestoreActorValue("Magicka", 1000.0)
    Utility.Wait(2.0)
    (v as VrmAt21ActorLog).Take()
    (w as VrmAt21ActorLog).Take()
    v.StartCombat(w)
    w.StartCombat(v)
    String r = "a Vigilant with " + Num(v.GetActorValue("Magicka")) + " magicka held 700 from a held fire warlock; its health/WardPower/magicka every second:"
    Int s = 0
    While s < 15
        Utility.Wait(1.0)
        r += " " + Num(v.GetActorValue("Health")) + "/" + Num(v.GetActorValue("WardPower")) + "/" + Num(v.GetActorValue("Magicka"))
        s += 1
    EndWhile
    r += "\n  the Vigilant's events " + (v as VrmAt21ActorLog).Take() + "\n  the warlock's " + (w as VrmAt21ActorLog).Take() + "\n"
    v.StopCombat()
    w.StopCombat()
    Gone(v)
    Gone(w)
    return r
EndFunction

; combat-fleeing: an Average NPC at 18% against a level 20 fighter for
; 20 s (autotest22: it didn't flee in 12), a Coward the same as the
; control; IsFleeing read at the end.
String Function FleeCase(ActorBase b, String label)
    Actor f = SpawnWh(b, 0.0, 0.0)
    Actor brute = SpawnWh(Level20FighterBase, 250.0, 0.0)
    f.SetActorValue("Health", 3000.0)
    f.ForceActorValue("HealRate", 0.0)
    Utility.Wait(1.5)
    f.DamageActorValue("Health", f.GetActorValue("Health") * 0.82)
    f.StartCombat(brute)
    brute.StartCombat(f)
    String r = label + " at " + Num(f.GetActorValuePercentage("Health")) + " set on a level 20 fighter; distance/state/health every 2 s:"
    Int s = 0
    While s < 10
        Utility.Wait(2.0)
        r += " " + Num(f.GetDistance(brute)) + "/" + f.GetCombatState() + "/" + Num(f.GetActorValuePercentage("Health"))
        s += 1
    EndWhile
    r += "; IsFleeing " + Probe(f, PS_Fleeing, PE_Fleeing) + "\n"
    brute.Kill(None)
    f.StopCombat()
    Gone(brute)
    Gone(f)
    Utility.Wait(1.0)
    return r
EndFunction

String Function TestFlee()
    return FleeCase(AverageBase, "an Average NPC") + FleeCase(CowardBase, "a Coward")
EndFunction

; What an NPC's view of the player is: hostile, detecting, combat state and
; target.
String Function Views(Actor a)
    return "hostile " + a.IsHostileToActor(Player) + ", detects " + Player.IsDetectedBy(a) + ", state " + a.GetCombatState() + ", targets the player " + (a.GetCombatTarget() == Player)
EndFunction

; factions: the player's SetAttackActorOnSight seen by NPCs of aggression
; 1, 2 and 3 (autotest22: aggression 1 ignored it).
String Function SightCase(ActorBase b, String label)
    Player.MoveTo(WhCenter, 0.0, 0.0, 0.0, false)
    Actor a = SpawnWh(b, 400.0, 0.0)
    Utility.Wait(4.0)
    String r = label + ": before, " + Views(a)
    Player.SetAttackActorOnSight(true)
    Utility.Wait(6.0)
    r += "; SetAttackActorOnSight +6 s, " + Views(a) + "\n"
    Player.SetAttackActorOnSight(false)
    a.StopCombat()
    Player.StopCombatAlarm()
    Gone(a)
    Player.RestoreActorValue("Health", 2000.0)
    Utility.Wait(1.0)
    return r
EndFunction

String Function TestAttackOnSightAggression()
    return SightCase(RedFighterBase, "aggression 1") + SightCase(VeryAggressiveBase, "aggression 2") + SightCase(FrenziedBase, "aggression 3")
EndFunction

; crime: a guard of a faction with Arrest and Attack on Sight, at bounties
; 0, 999 and 1000 (autotest22's faction had no Attack on Sight).
String Function TestAttackOnSightCrime()
    Actor g = SpawnWh(AosGuardBase, 500.0, 0.0)
    Player.MoveTo(WhCenter, 0.0, 0.0, 0.0, false)
    Utility.Wait(3.0)
    String r = "a guard of a faction with Attack on Sight, 500 off: bounty 0, " + Views(g) + "\n"
    AosFaction.SetCrimeGold(999)
    Utility.Wait(6.0)
    r += "bounty 999 +6 s: " + Views(g) + "\n"
    AosFaction.SetCrimeGold(1000)
    Utility.Wait(6.0)
    r += "bounty 1000 +6 s: " + Views(g) + "\n"
    AosFaction.SetCrimeGold(0)
    g.StopCombat()
    Player.StopCombatAlarm()
    Gone(g)
    Player.RestoreActorValue("Health", 2000.0)
    return r
EndFunction

; magic: blows on an ethereal NPC, and an NPC set on an ethereal player.
String Function TestEthereal()
    Actor t = SpawnWh(LoggedBase, 150.0, 0.0)
    Actor a = SpawnWh(RedFighterBase, 0.0, 0.0)
    t.SetDontMove(true)
    t.ForceActorValue("HealRate", 0.0)
    Utility.Wait(1.0)
    EtherealSpell.Cast(t, t)
    Utility.Wait(1.0)
    (t as VrmAt21ActorLog).Take()
    a.StartCombat(t)
    String r = "a fighter on an ethereal NPC: its health by second:"
    Int s = 0
    While s < 6
        Utility.Wait(1.0)
        r += " " + Num(t.GetActorValue("Health"))
        s += 1
    EndWhile
    r += "; the target's events " + (t as VrmAt21ActorLog).Take() + "\n"
    a.StopCombat()
    t.StopCombat()
    Gone(t)
    Player.MoveTo(WhCenter, 0.0, 0.0, 0.0, false)
    Utility.Wait(1.0)
    EtherealSpell.Cast(Player, Player)
    Utility.Wait(1.0)
    a.MoveTo(WhCenter, 300.0, 0.0, 0.0, false)
    a.StartCombat(Player)
    r += "a fighter set on the ethereal player: state/distance/player health by second:"
    s = 0
    While s < 6
        Utility.Wait(1.0)
        r += " " + a.GetCombatState() + "/" + Num(a.GetDistance(Player)) + "/" + Num(Player.GetActorValue("Health"))
        s += 1
    EndWhile
    r += "\n"
    a.StopCombat()
    Player.StopCombatAlarm()
    Gone(a)
    Player.DispelSpell(EtherealSpell)
    Player.RestoreActorValue("Health", 2000.0)
    return r
EndFunction

; combat-assistance: helpers of no faction at relationship ranks 1, 2 and 4
; with a victim (rank 3 joined in autotest21), helping allies only and
; friends and allies.
String Function HelpCase(ActorBase hb, Int rank, String label)
    Actor v = SpawnWh(NakedBase, 0.0, 0.0)
    Actor a = SpawnWh(RedFighterBase, 250.0, 0.0)
    Actor h = SpawnWh(hb, -400.0, 0.0)
    v.SetDontMove(true)
    v.ForceActorValue("Health", 5000.0)
    a.ForceActorValue("Health", 5000.0)
    h.SetRelationshipRank(v, rank)
    v.SetRelationshipRank(h, rank)
    Utility.Wait(1.5)
    a.StartCombat(v)
    Utility.Wait(8.0)
    String r = "  " + label + ", rank " + h.GetRelationshipRank(v) + ": victim state " + v.GetCombatState() + ", helper state " + h.GetCombatState() + ", helper targets the attacker " + (h.GetCombatTarget() == a) + "\n"
    a.StopCombat()
    v.StopCombat()
    h.StopCombat()
    Gone(a)
    Gone(v)
    Gone(h)
    Utility.Wait(1.0)
    return r
EndFunction

String Function TestRelationshipHelp()
    String r = "a Red fighter attacks a held victim; a helper 650 off:\n"
    r += HelpCase(HelperAlliesBase, 1, "helps allies")
    r += HelpCase(HelperAlliesBase, 2, "helps allies")
    r += HelpCase(HelperAlliesBase, 3, "helps allies")
    r += HelpCase(HelperAlliesBase, 4, "helps allies")
    r += HelpCase(HelperBase, 1, "helps friends and allies")
    r += HelpCase(HelperBase, 2, "helps friends and allies")
    r += HelpCase(HelperBase, 4, "helps friends and allies")
    return r
EndFunction

; combat-assistance: a CurrentFollowerFaction member, not a teammate, when
; the player is attacked.
String Function FollowerCase(ActorBase hb, String label)
    Player.MoveTo(WhCenter, 0.0, 0.0, 0.0, false)
    Actor h = SpawnWh(hb, -300.0, 0.0)
    Actor a = SpawnWh(RedFighterBase, 300.0, 0.0)
    h.AddToFaction(FollowerFaction)
    a.ForceActorValue("Health", 5000.0)
    Utility.Wait(2.0)
    a.StartCombat(Player)
    Utility.Wait(8.0)
    String r = "  " + label + " (rank " + h.GetFactionRank(FollowerFaction) + ", teammate " + h.IsPlayerTeammate() + "): the attacker targets the player " + (a.GetCombatTarget() == Player) + "; helper state " + h.GetCombatState() + ", targets the attacker " + (h.GetCombatTarget() == a) + "\n"
    a.StopCombat()
    h.StopCombat()
    Player.StopCombatAlarm()
    h.RemoveFromFaction(FollowerFaction)
    Gone(a)
    Gone(h)
    Player.RestoreActorValue("Health", 2000.0)
    Utility.Wait(1.0)
    return r
EndFunction

String Function TestFollowerHelp()
    return "a Red fighter attacks the player; a follower-faction member 300 off:\n" + FollowerCase(HelperBase, "helps friends and allies") + FollowerCase(HelperAlliesBase, "helps allies")
EndFunction

String Function TestLeaveWarehouse()
    Player.ModActorValue("Health", -1000.0)
    GoCell()
    return "back in the test cell\n"
EndFunction
