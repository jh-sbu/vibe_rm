; Items, statistics, actor values and loose objects, in the test cell.

; actor-values: stats of vanilla NPCs placed here.
String Function TestNpcStats()
    Actor k = Spawn(KhaylaBase, 0.0, 300.0)
    Actor d = Spawn(DerveninBase, 200.0, 300.0)
    Actor deer = Spawn(DeerBase, 400.0, 300.0)
    Actor arniel = Spawn(ArnielSummonBase, -200.0, 300.0)
    Actor dremora = Spawn(PCMultDremora, -400.0, 300.0)
    k.SetDontMove(true)
    d.SetDontMove(true)
    deer.SetDontMove(true)
    arniel.SetDontMove(true)
    dremora.SetDontMove(true)
    Utility.Wait(1.0)
    String r = "Khayla (non-auto, health offset 105): base health " + k.GetBaseActorValue("Health") + ", level " + k.GetLevel() + "\n"
    r += "Dervenin: OneHanded " + d.GetBaseActorValue("OneHanded") + " Marksman " + d.GetBaseActorValue("Marksman") + " Block " + d.GetBaseActorValue("Block")
    r += " Enchanting " + d.GetBaseActorValue("Enchanting") + " Alteration " + d.GetBaseActorValue("Alteration") + " Alchemy " + d.GetBaseActorValue("Alchemy") + ", level " + d.GetLevel() + "\n"
    r += "deer (Sneak offset 70): Sneak " + deer.GetBaseActorValue("Sneak") + ", level " + deer.GetLevel() + "\n"
    r += "MGArnielSummon: Alteration " + arniel.GetBaseActorValue("Alteration") + ", level " + arniel.GetLevel() + "\n"
    r += "a PC level mult copy of EncDremoraMelee06 (calc min 46): level " + dremora.GetLevel() + ", OneHanded " + dremora.GetBaseActorValue("OneHanded") + " TwoHanded " + dremora.GetBaseActorValue("TwoHanded") + " HeavyArmor " + dremora.GetBaseActorValue("HeavyArmor") + " Block " + dremora.GetBaseActorValue("Block") + " Destruction " + dremora.GetBaseActorValue("Destruction") + " health " + dremora.GetBaseActorValue("Health") + "\n"
    Gone(k)
    Gone(d)
    Gone(deer)
    Gone(arniel)
    Gone(dremora)
    return r
EndFunction

; journal: which statistics a kill by script (Kill(player)) counts.
String Function StatLine()
    return "people " + Game.QueryStat("People Killed") + " animals " + Game.QueryStat("Animals Killed") + " creatures " + Game.QueryStat("Creatures Killed") + " undead " + Game.QueryStat("Undead Killed") + " daedra " + Game.QueryStat("Daedra Killed") + " automatons " + Game.QueryStat("Automatons Killed") + " bunnies " + Game.QueryStat("Bunnies Slaughtered")
EndFunction

String Function KillFor(ActorBase b, String label)
    Actor a = Spawn(b, 0.0, 400.0)
    a.SetDontMove(true)
    Utility.Wait(1.0)
    a.Kill(Player)
    Utility.Wait(1.0)
    String r = "  " + label + ": " + StatLine() + "\n"
    Gone(a)
    return r
EndFunction

String Function TestKillStats()
    String r = "before: " + StatLine() + "\n"
    r += KillFor(WolfBase, "a wolf")
    r += KillFor(HareBase, "a hare")
    r += KillFor(MudcrabBase, "a mudcrab")
    r += KillFor(SkeletonBase, "a skeleton")
    r += KillFor(AtronachBase, "a flame atronach")
    r += KillFor(SpiderBase, "a dwarven spider")
    r += KillFor(VampireBase, "a vampire")
    r += KillFor(NakedBase, "a commoner (no crime faction)")
    return r
EndFunction

String Function TestUseStats()
    String r = "Skill Increases " + Game.QueryStat("Skill Increases")
    Game.IncrementSkill("Alchemy")
    r += ", IncrementSkill: " + Game.QueryStat("Skill Increases")
    Game.AdvanceSkill("Alchemy", 100000.0)
    r += ", AdvanceSkill (a level's worth): " + Game.QueryStat("Skill Increases") + "\n"
    r += "Food Eaten " + Game.QueryStat("Food Eaten") + ", Ingredients Eaten " + Game.QueryStat("Ingredients Eaten") + ", Potions Used " + Game.QueryStat("Potions Used")
    Player.AddItem(SweetRoll, 1, true)
    Player.EquipItem(SweetRoll, false, true)
    Player.AddItem(Wheat, 1, true)
    Player.EquipItem(Wheat, false, true)
    Player.AddItem(PotionHealth, 1, true)
    Player.EquipItem(PotionHealth, false, true)
    Utility.Wait(1.0)
    r += "; EquipItem a sweet roll, wheat, a potion: " + Game.QueryStat("Food Eaten") + ", " + Game.QueryStat("Ingredients Eaten") + ", " + Game.QueryStat("Potions Used") + "\n"
    return r
EndFunction

String Function TestBoundWeapon()
    BoundSwordSpell.Cast(Player, Player)
    Utility.Wait(2.0)
    String r = "Bound Sword cast on the player: count " + Player.GetItemCount(BoundSword) + ", equipped " + Player.IsEquipped(BoundSword)
    ObjectReference c = Put(EmptyChest, 200.0, 0.0, 0.0)
    Player.RemoveItem(BoundSword, 1, true, c)
    Utility.Wait(0.5)
    r += "; RemoveItem into a chest: player " + Player.GetItemCount(BoundSword) + ", chest " + c.GetItemCount(BoundSword)
    Player.DispelSpell(BoundSwordSpell)
    Utility.Wait(1.0)
    r += "; dispelled: player " + Player.GetItemCount(BoundSword) + ", chest " + c.GetItemCount(BoundSword) + "\n"
    Gone(c)
    return r
EndFunction

; crime.md / followers.md: NPCs taking up armor given by script.
String Function TestTakeUpArmor()
    Actor a = Spawn(NakedBase, 0.0, 300.0)
    Actor t = Spawn(NakedBase, 300.0, 300.0)
    t.SetPlayerTeammate(true, true)
    Utility.Wait(1.0)
    a.AddItem(LeatherCuirass, 1, true)
    t.AddItem(LeatherCuirass, 1, true)
    Utility.Wait(3.0)
    String r = "leather cuirass given: NPC wears it " + a.IsEquipped(LeatherCuirass) + ", teammate " + t.IsEquipped(LeatherCuirass)
    Utility.Wait(12.0)
    r += "; 15 s on: " + a.IsEquipped(LeatherCuirass) + ", " + t.IsEquipped(LeatherCuirass)
    a.AddItem(IronCuirass, 1, true)
    t.AddItem(IronCuirass, 1, true)
    Utility.Wait(3.0)
    r += "; an iron one too: NPC iron " + a.IsEquipped(IronCuirass) + " leather " + a.IsEquipped(LeatherCuirass) + ", teammate iron " + t.IsEquipped(IronCuirass) + " leather " + t.IsEquipped(LeatherCuirass) + "\n"
    t.SetPlayerTeammate(false, false)
    Gone(a)
    Gone(t)
    return r
EndFunction

String Function TestDisarm()
    Actor a = Spawn(LoggedBase, 0.0, 250.0)
    a.AddItem(IronSword, 1, true)
    a.EquipItem(IronSword, false, true)
    a.SetDontMove(true)
    Utility.Wait(1.0)
    a.SetAlert(true)
    a.DrawWeapon()
    Utility.Wait(2.0)
    (a as VrmAt21ActorLog).Take()
    String r = "sword drawn " + a.IsWeaponDrawn()
    DisarmSpell.Cast(Player, a)
    Utility.Wait(2.0)
    r += "; Disarm (shout spell) cast by script: still holds " + Id(a.GetEquippedWeapon(false)) + ", count " + a.GetItemCount(IronSword) + ", events " + (a as VrmAt21ActorLog).Take()
    ObjectReference s = Game.FindClosestReferenceOfTypeFromRef(IronSword, a, 1000.0)
    If s != None
        r += "; on the ground " + Num(a.GetDistance(s)) + " from it (dx " + Num(s.GetPositionX() - a.GetPositionX()) + ", dy " + Num(s.GetPositionY() - a.GetPositionY()) + ", dz " + Num(s.GetPositionZ() - a.GetPositionZ()) + ")"
    EndIf
    r += "\n"
    Actor b = Spawn(LoggedBase, 300.0, 250.0)
    b.AddItem(IronGreatsword, 1, true)
    b.EquipItem(IronGreatsword, false, true)
    b.SetDontMove(true)
    Utility.Wait(1.0)
    DisarmSpell.Cast(Player, b)
    Utility.Wait(2.0)
    r += "sheathed greatsword, Disarm: holds " + Id(b.GetEquippedWeapon(false)) + ", count " + b.GetItemCount(IronGreatsword) + "\n"
    a.StopCombat()
    b.StopCombat()
    Gone(a)
    Gone(b)
    Gone(s)
    return r
EndFunction

; loose-objects
String Function TestLooseObjects()
    String r = "plugin bowl placed 150 above the floor: z " + Num(FloatingBowl.GetPositionZ() - CellCenter.GetPositionZ())
    Utility.Wait(2.0)
    r += ", 2 s on " + Num(FloatingBowl.GetPositionZ() - CellCenter.GetPositionZ())
    FloatingBowl.ApplyHavokImpulse(0.0, 0.0, 1.0, 0.1)
    Utility.Wait(2.0)
    r += ", nudged: " + Num(FloatingBowl.GetPositionZ() - CellCenter.GetPositionZ()) + "\n"
    ObjectReference b = Put(Bowl, 300.0, 300.0, 150.0)
    Utility.Wait(2.0)
    r += "PlaceAtMe'd bowl 150 up: 2 s on " + Num(b.GetPositionZ() - CellCenter.GetPositionZ()) + ", mass " + b.GetMass() + "\n"
    Float[] mags = new Float[3]
    mags[0] = 1.0
    mags[1] = 10.0
    mags[2] = 50.0
    Int i = 0
    While i < 3
        ObjectReference p = Put(Bowl, -300.0, -300.0 + 200.0 * i, 0.0)
        Utility.Wait(1.5)
        Float x = p.GetPositionX()
        p.ApplyHavokImpulse(1.0, 0.0, 0.0, mags[i])
        Utility.Wait(0.5)
        Float x1 = p.GetPositionX()
        Utility.Wait(2.5)
        r += "bowl given ApplyHavokImpulse(+x, " + mags[i] + "): moved " + Num(x1 - x) + " in 0.5 s, " + Num(p.GetPositionX() - x) + " in 3 s\n"
        Gone(p)
        i += 1
    EndWhile
    Player.AddItem(IronSword, 1, true)
    Player.MoveTo(CellCenter, 0.0, -400.0, 0.0, false)
    Player.SetAngle(0.0, 0.0, 0.0)
    Utility.Wait(1.0)
    ObjectReference dropped = Player.DropObject(IronSword, 1)
    Utility.Wait(0.1)
    r += "the player (facing north) drops a sword: at dx " + Num(dropped.GetPositionX() - Player.GetPositionX()) + ", dy " + Num(dropped.GetPositionY() - Player.GetPositionY()) + ", dz " + Num(dropped.GetPositionZ() - Player.GetPositionZ())
    Utility.Wait(2.0)
    r += "; 2 s on dz " + Num(dropped.GetPositionZ() - Player.GetPositionZ()) + "\n"
    Gone(dropped)
    Gone(b)
    Actor c = Spawn(NakedBase, 300.0, -300.0)
    Utility.Wait(1.0)
    Float cx = c.GetPositionX()
    c.Kill(None)
    Utility.Wait(1.0)
    c.PushActorAway(c, 5.0)
    Utility.Wait(3.0)
    r += "a corpse pushed away: moved " + Num(c.GetPositionX() - cx) + " in x, z " + Num(c.GetPositionZ() - CellCenter.GetPositionZ()) + "\n"
    Gone(c)
    GoCell()
    return r
EndFunction
