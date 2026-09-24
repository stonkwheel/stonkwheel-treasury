//! STONKWHEEL Treasury: prepare (3), launch token accounts (0),
//! activate (2), permissionless buyback-and-burn (1), clean uncreated mint (4).
//! There is deliberately no principal transfer, delegation or administration.
#![allow(unexpected_cfgs)]
use solana_program::{account_info::AccountInfo, bpf_loader_upgradeable, entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction}, msg, program::{invoke, invoke_signed},
    program_error::ProgramError, pubkey, pubkey::Pubkey, system_instruction, system_program,
    sysvar::{clock::Clock, rent::Rent, Sysvar}};
#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);
const WSOL: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
const TOKEN: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const TOKEN22: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
const ATA: Pubkey = pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
const CPMM: Pubkey = pubkey!("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C");
const LAUNCH: Pubkey = pubkey!("LanMV9sAd7wArD4vJFi2qDdfnVhFxYSUg6eADduJ3uj");
const PLATFORM: Pubkey = pubkey!("6BwHHDg3u1854jC8PDLXvR4spTcLNaoBxLJNGC4nTESt");
const GLOBAL: Pubkey = pubkey!("6s1xP3hpbAfFoNtUNF8mfHsjr2Bd97JxFJRWLbL6aHuX");
const CP_CONFIG: Pubkey = pubkey!("CRRS5ieQmBrZjWhcj99JuGrT5tyuWDaGAXLXLFjbAtjQ");
const WITHHELD: Pubkey = pubkey!("5KXDF6QnqhBj72hDtJNkkpFaQVUfbFXNybMsp3DiK6tD");
const SUPPLY: u64 = 1_000_000_000_000_000;
const PRINCIPAL: u64 = 300_000_000_000_000;
const CONFIG_LEN: usize = 160;
const STATS_LEN: usize = 112;
const CONFIG_MAGIC: &[u8;8] = b"STNKPR02";
const STATS_MAGIC: &[u8;8] = b"STNKST01";
fn require(x: bool) -> ProgramResult { if x { Ok(()) } else { Err(ProgramError::InvalidAccountData) } }
fn key(a: &AccountInfo, k: &Pubkey) -> ProgramResult { require(a.key == k) }
fn executable(a: &AccountInfo, k: &Pubkey) -> ProgramResult { key(a,k)?; require(a.executable) }
fn system_wallet(a: &AccountInfo) -> ProgramResult { require(a.owner == &system_program::id() && a.data_is_empty()) }
fn u64_at(d: &[u8], n: usize) -> Result<u64,ProgramError> { Ok(u64::from_le_bytes(d.get(n..n+8).ok_or(ProgramError::InvalidAccountData)?.try_into().unwrap())) }
fn pk(d: &[u8], n: usize) -> Pubkey { Pubkey::new_from_array(d[n..n+32].try_into().unwrap()) }
fn pda(a: &AccountInfo, seed: &[u8], program: &Pubkey) -> Result<u8,ProgramError> {
    let (p,b) = Pubkey::find_program_address(&[seed],program); key(a,&p)?; Ok(b)
}
fn derived(seeds: &[&[u8]], program: &Pubkey) -> Pubkey { Pubkey::find_program_address(seeds, program).0 }
fn associated(owner: &Pubkey, mint: &Pubkey, token: &Pubkey) -> Pubkey { derived(&[owner.as_ref(),token.as_ref(),mint.as_ref()],&ATA) }
fn token_account(a: &AccountInfo, owner: &Pubkey, mint: &Pubkey, token: &Pubkey) -> ProgramResult {
    key(a,&associated(owner,mint,token))?; require(a.owner == token)?;
    let d=a.try_borrow_data()?;
    require(d.len()>=165 && &d[..32]==mint.as_ref() && &d[32..64]==owner.as_ref()
        && d[108]==1 && d[72..76]==[0;4] && d[129..133]==[0;4])
}
fn mint(a: &AccountInfo, expected: &Pubkey, owner: &Pubkey, decimals: u8) -> ProgramResult {
    key(a,expected)?; require(a.owner==owner)?; let d=a.try_borrow_data()?;
    require(d.len()>=82 && d[44]==decimals && d[45]==1)
}
fn validate_launch_mint(a: &AccountInfo, expected: &Pubkey) -> ProgramResult {
    mint(a,expected,&TOKEN22,6)?; let d=a.try_borrow_data()?;
    require(d[..4]==[0;4] && d[46..50]==[0;4] && u64_at(&d,36)?==SUPPLY && d.len()>166 && d[165]==1)?;
    let mut p=166; let mut found=0u8;
    while p+4<=d.len() {
        let kind=u16::from_le_bytes(d[p..p+2].try_into().unwrap());
        let len=u16::from_le_bytes(d[p+2..p+4].try_into().unwrap()) as usize; p+=4;
        if kind==0 { require(d[p-4..].iter().all(|b|*b==0))?; break; }
        require(p+len<=d.len())?; let e=&d[p..p+len];
        match kind {
            1 => { require(found&1==0 && len==108)?; found|=1;
                // LaunchLab retains the fee-configuration PDA; Stonkfun harvests withheld fees.
                require(&e[..32]==derived(&[b"vault_auth_seed"],&LAUNCH).as_ref() && &e[32..64]==WITHHELD.as_ref())?;
                for offset in [72,90] { require(u64_at(e,offset+8)?==SUPPLY && e[offset+16..offset+18]==300u16.to_le_bytes())?; }
            },
            18 => { require(found&2==0 && len==64 && &e[32..64]==expected.as_ref())?; found|=2; },
            19 => { require(found&4==0 && len>=64 && &e[32..64]==expected.as_ref())?; found|=4; },
            _ => return Err(ProgramError::InvalidAccountData),
        }
        p+=len;
    }
    require(found==7)
}
#[derive(Clone,Copy)]
struct Config { active: bool, nonce:u8, mint:Pubkey, operations:Pubkey, cp_config:Pubkey }
fn config(a: &AccountInfo, program: &Pubkey) -> Result<Config,ProgramError> {
    pda(a,b"config",program)?; require(a.owner==program)?; let d=a.try_borrow_data()?;
    require(d.len()==CONFIG_LEN && &d[..8]==CONFIG_MAGIC && d[8]<=1)?;
    Ok(Config { active:d[8]==1, nonce:d[9], mint:pk(&d,16),operations:pk(&d,48),cp_config:pk(&d,80) })
}
fn loader(program: &Pubkey, executable_program:&AccountInfo, data:&AccountInfo, authority:Option<&Pubkey>) -> ProgramResult {
    executable(executable_program,program)?;
    require(executable_program.owner==&bpf_loader_upgradeable::id() && data.owner==&bpf_loader_upgradeable::id())?;
    let p=executable_program.try_borrow_data()?; let d=data.try_borrow_data()?;
    require(p.len()==36 && p[..4]==[2,0,0,0] && &p[4..36]==data.key.as_ref() && d.len()>=45 && d[..4]==[3,0,0,0])?;
    match authority { Some(k)=>require(d[12]==1 && &d[13..45]==k.as_ref()), None=>require(d[12]==0) }
}
fn ms_seed(nonce:u8)->String { format!("stonkwheel-ms-{}",nonce) }
fn multisig(a:&AccountInfo,holder:&Pubkey,nonce:u8)->ProgramResult {
    key(a,&Pubkey::create_with_seed(holder,&ms_seed(nonce),&TOKEN)?)?; require(a.owner==&TOKEN)?;
    let d=a.try_borrow_data()?; require(d.len()==355 && d[..3]==[1,1,1] && &d[3..35]==holder.as_ref() && d[35..].iter().all(|b|*b==0))
}
fn transfer_sol<'a>(from:&AccountInfo<'a>,to:&AccountInfo<'a>,system:&AccountInfo<'a>,amount:u64,seeds:&[&[u8]])->ProgramResult {
    if amount==0 { return Ok(()); }
    invoke_signed(&system_instruction::transfer(from.key,to.key,amount),&[from.clone(),to.clone(),system.clone()],&[seeds])
}
fn create_ata<'a>(payer:&AccountInfo<'a>,account:&AccountInfo<'a>,owner:&AccountInfo<'a>,mint:&AccountInfo<'a>,token:&AccountInfo<'a>,system:&AccountInfo<'a>,ata:&AccountInfo<'a>,seeds:&[&[u8]])->ProgramResult {
    key(account,&associated(owner.key,mint.key,token.key))?;
    let ix=Instruction { program_id:ATA,data:vec![1],accounts:vec![AccountMeta::new(*payer.key,true),AccountMeta::new(*account.key,false),
        AccountMeta::new_readonly(*owner.key,false),AccountMeta::new_readonly(*mint.key,false),AccountMeta::new_readonly(system_program::id(),false),AccountMeta::new_readonly(*token.key,false)] };
    let infos=[payer.clone(),account.clone(),owner.clone(),mint.clone(),system.clone(),token.clone(),ata.clone()];
    if seeds.is_empty() { invoke(&ix,&infos) } else { invoke_signed(&ix,&infos,&[seeds]) }
}
fn allocate_pda<'a>(payer:&AccountInfo<'a>,a:&AccountInfo<'a>,system:&AccountInfo<'a>,program:&Pubkey,seed:&[u8],len:usize)->ProgramResult {
    system_wallet(a)?; let bump=[pda(a,seed,program)?]; let seeds:&[&[u8]]=&[seed,&bump];
    let needed=Rent::get()?.minimum_balance(len).saturating_sub(a.lamports());
    if needed>0 { invoke(&system_instruction::transfer(payer.key,a.key,needed),&[payer.clone(),a.clone(),system.clone()])?; }
    invoke_signed(&system_instruction::allocate(a.key,len as u64),&[a.clone(),system.clone()],&[seeds])?;
    invoke_signed(&system_instruction::assign(a.key,program),&[a.clone(),system.clone()],&[seeds])
}
fn stats(a:&AccountInfo,program:&Pubkey,config:&Pubkey)->ProgramResult {
    pda(a,b"stats",program)?;require(a.owner==program)?;let d=a.try_borrow_data()?;
    require(d.len()==STATS_LEN && &d[..8]==STATS_MAGIC && &d[8..40]==config.as_ref())
}
fn launch_pool(a:&AccountInfo,c:&Config,status:u8)->ProgramResult {
    key(a,&derived(&[b"pool",c.mint.as_ref(),WSOL.as_ref()],&LAUNCH))?;require(a.owner==&LAUNCH)?;
    let d=a.try_borrow_data()?;
    require(d.len()==429 && d[17]==status && d[18]==6 && d[19]==9 && d[20]==1
        && u64_at(&d,21)?==SUPPLY && u64_at(&d,29)?==793_100_000_000_000
        && u64_at(&d,69)?==85_000_000_000 && u64_at(&d,101)?==0
        && &d[141..173]==GLOBAL.as_ref() && &d[173..205]==PLATFORM.as_ref()
        && &d[205..237]==c.mint.as_ref() && &d[237..269]==WSOL.as_ref())
}
// payer, config, holder, burn, multisig, rewards-wSOL, working-wSOL, stats,
// wSOL mint, System, Token, ATA, executable programme, ProgramData, upgrade signer.
fn prepare(program:&Pubkey,a:&[AccountInfo],nonce:u8,mint_key:Pubkey)->ProgramResult {
    require(a.len()==15 && a[0].is_signer && a[14].is_signer && a[14].key!=a[0].key)?;system_wallet(&a[0])?;
    require(mint_key!=WSOL && mint_key!=Pubkey::default())?;
    loader(program,&a[12],&a[13],Some(a[14].key))?;
    for (i,k) in [(9,system_program::id()),(10,TOKEN),(11,ATA)] { executable(&a[i],&k)?; }
    mint(&a[8],&WSOL,&TOKEN,9)?;
    let bump=[pda(&a[2],b"holder",program)?];let seeds:&[&[u8]]=&[b"holder",&bump];
    pda(&a[3],b"burn",program)?;system_wallet(&a[2])?;system_wallet(&a[3])?;
    allocate_pda(&a[0],&a[1],&a[9],program,b"config",CONFIG_LEN)?;
    allocate_pda(&a[0],&a[7],&a[9],program,b"stats",STATS_LEN)?;
    let rent=Rent::get()?;
    let topup=rent.minimum_balance(0).saturating_sub(a[2].lamports());
    if topup>0 { invoke(&system_instruction::transfer(a[0].key,a[2].key,topup),&[a[0].clone(),a[2].clone(),a[9].clone()])?; }
    let seed=ms_seed(nonce);key(&a[4],&Pubkey::create_with_seed(a[2].key,&seed,&TOKEN)?)?;
    system_wallet(&a[4])?;
    let needed=rent.minimum_balance(355).saturating_sub(a[4].lamports());
    if needed>0 { invoke(&system_instruction::transfer(a[0].key,a[4].key,needed),&[a[0].clone(),a[4].clone(),a[9].clone()])?; }
    invoke_signed(&system_instruction::allocate_with_seed(a[4].key,a[2].key,&seed,355,&TOKEN),&[a[4].clone(),a[2].clone(),a[9].clone()],&[seeds])?;
    invoke(&Instruction { program_id:TOKEN,data:vec![19,1],accounts:vec![AccountMeta::new(*a[4].key,false),AccountMeta::new_readonly(*a[2].key,false)] },&[a[4].clone(),a[2].clone(),a[10].clone()])?;
    multisig(&a[4],a[2].key,nonce)?;
    create_ata(&a[0],&a[5],&a[4],&a[8],&a[10],&a[9],&a[11],&[])?;
    create_ata(&a[0],&a[6],&a[2],&a[8],&a[10],&a[9],&a[11],&[])?;
    { let mut d=a[1].try_borrow_mut_data()?;d[..8].copy_from_slice(CONFIG_MAGIC);d[9]=nonce;
      d[16..48].copy_from_slice(mint_key.as_ref());d[48..80].copy_from_slice(a[0].key.as_ref());d[80..112].copy_from_slice(CP_CONFIG.as_ref());
      d[128..136].copy_from_slice(&Clock::get()?.unix_timestamp.to_le_bytes()); }
    { let mut d=a[7].try_borrow_mut_data()?;d[..8].copy_from_slice(STATS_MAGIC);d[8..40].copy_from_slice(a[1].key.as_ref()); }
    msg!("STONKWHEEL_PREPARED");Ok(())
}
// payer, config, holder, burn, multisig, principal, bought, mint, System, Token22, ATA.
fn token_setup(program:&Pubkey,a:&[AccountInfo])->ProgramResult {
    require(a.len()==11)?;let c=config(&a[1],program)?;
    require(!c.active && a[0].is_signer && a[0].key==&c.operations)?;
    pda(&a[2],b"holder",program)?;pda(&a[3],b"burn",program)?;multisig(&a[4],a[2].key,c.nonce)?;
    mint(&a[7],&c.mint,&TOKEN22,6)?;
    for (i,k) in [(8,system_program::id()),(9,TOKEN22),(10,ATA)] { executable(&a[i],&k)?; }
    create_ata(&a[0],&a[5],&a[4],&a[7],&a[9],&a[8],&a[10],&[])?;
    create_ata(&a[0],&a[6],&a[3],&a[7],&a[9],&a[8],&a[10],&[])
}
// deployer, config, holder, burn, multisig, principal, bought, mint,
// programme, ProgramData, original LaunchLab pool, stats, upgrade signer.
fn activate(program:&Pubkey,a:&[AccountInfo])->ProgramResult {
    require(a.len()==13 && a[12].is_signer && a[12].key!=a[0].key)?;let c=config(&a[1],program)?;
    require(!c.active && a[0].is_signer && a[0].key==&c.operations)?;
    // Retain the separate upgrade authority through the first verified cycle.
    // The ordinary keeper never needs this signer for permissionless execution.
    loader(program,&a[8],&a[9],Some(a[12].key))?;
    pda(&a[2],b"holder",program)?;pda(&a[3],b"burn",program)?;
    multisig(&a[4],a[2].key,c.nonce)?;
    token_account(&a[5],a[4].key,&c.mint,&TOKEN22)?;token_account(&a[6],a[3].key,&c.mint,&TOKEN22)?;
    require(u64_at(&a[5].try_borrow_data()?,64)?==PRINCIPAL && u64_at(&a[6].try_borrow_data()?,64)?==0)?;
    validate_launch_mint(&a[7],&c.mint)?;launch_pool(&a[10],&c,0)?;stats(&a[11],program,a[1].key)?;
    let clock=Clock::get()?;let mut d=a[1].try_borrow_mut_data()?;d[8]=1;
    d[112..120].copy_from_slice(&clock.slot.to_le_bytes());d[120..128].copy_from_slice(&clock.unix_timestamp.to_le_bytes());
    msg!("STONKWHEEL_ACTIVE;principal={};buyback_bps=9000",PRINCIPAL);Ok(())
}
fn pool(a:&[AccountInfo],c:&Config)->ProgramResult {
    let (m0,m1)=if c.mint<WSOL {(c.mint,WSOL)} else {(WSOL,c.mint)};
    let pool=derived(&[b"pool",c.cp_config.as_ref(),m0.as_ref(),m1.as_ref()],&CPMM);
    key(&a[6],&pool)?;key(&a[7],&derived(&[b"vault_and_lp_mint_auth_seed"],&CPMM))?;key(&a[8],&c.cp_config)?;
    key(&a[9],&derived(&[b"pool_vault",pool.as_ref(),WSOL.as_ref()],&CPMM))?;
    key(&a[10],&derived(&[b"pool_vault",pool.as_ref(),c.mint.as_ref()],&CPMM))?;
    key(&a[11],&derived(&[b"observation",pool.as_ref()],&CPMM))?;
    require(a[6].owner==&CPMM && a[8].owner==&CPMM && a[11].owner==&CPMM && a[9].owner==&TOKEN && a[10].owner==&TOKEN22)?;
    let d=a[6].try_borrow_data()?;
    require(d.len()==637 && &d[8..40]==c.cp_config.as_ref() && &d[168..200]==m0.as_ref() && &d[200..232]==m1.as_ref()
        && &d[296..328]==a[11].key.as_ref() && d[329]&4==0 && Clock::get()?.unix_timestamp>=0
        && Clock::get()?.unix_timestamp as u64>=u64_at(&d,373)?)?;
    let (v0,v1,t0,t1)=if m0==WSOL {(a[9].key,a[10].key,TOKEN,TOKEN22)} else {(a[10].key,a[9].key,TOKEN22,TOKEN)};
    require(&d[72..104]==v0.as_ref() && &d[104..136]==v1.as_ref() && &d[232..264]==t0.as_ref() && &d[264..296]==t1.as_ref())
}
// config, holder, working-wSOL, burn, bought, fixed operations, CPMM pool,
// authority, AMM config, SOL vault, token vault, observation, wSOL mint, mint,
// System, Token, Token22, ATA, CPMM, multisig, reward-wSOL, LaunchLab pool, stats.
// The principal account is deliberately absent.
fn execute(program:&Pubkey,a:&[AccountInfo])->ProgramResult {
    require(a.len()==23)?;let c=config(&a[0],program)?;require(c.active)?;
    let (holder,wsol,burn,bought)=(&a[1],&a[2],&a[3],&a[4]);
    let hb=[pda(holder,b"holder",program)?];let bb=[pda(burn,b"burn",program)?];
    let hs:&[&[u8]]=&[b"holder",&hb];let bs:&[&[u8]]=&[b"burn",&bb];
    system_wallet(holder)?;system_wallet(burn)?;key(&a[5],&c.operations)?;
    token_account(wsol,holder.key,&WSOL,&TOKEN)?;token_account(bought,burn.key,&c.mint,&TOKEN22)?;
    multisig(&a[19],holder.key,c.nonce)?;token_account(&a[20],a[19].key,&WSOL,&TOKEN)?;
    mint(&a[12],&WSOL,&TOKEN,9)?;mint(&a[13],&c.mint,&TOKEN22,6)?;
    for (i,k) in [(14,system_program::id()),(15,TOKEN),(16,TOKEN22),(17,ATA),(18,CPMM)] { executable(&a[i],&k)?; }
    launch_pool(&a[21],&c,2)?;pool(a,&c)?;stats(&a[22],program,a[0].key)?;
    invoke(&Instruction {program_id:TOKEN,data:vec![17],accounts:vec![AccountMeta::new(*a[20].key,false)]},&[a[20].clone(),a[15].clone()])?;
    let reward=u64_at(&a[20].try_borrow_data()?,64)?;
    if reward>0 {
        let mut data=vec![12];data.extend_from_slice(&reward.to_le_bytes());data.push(9);
        invoke_signed(&Instruction {program_id:TOKEN,data,accounts:vec![AccountMeta::new(*a[20].key,false),AccountMeta::new_readonly(WSOL,false),
            AccountMeta::new(*wsol.key,false),AccountMeta::new_readonly(*a[19].key,false),AccountMeta::new_readonly(*holder.key,true)]},
            &[a[20].clone(),a[12].clone(),wsol.clone(),a[19].clone(),holder.clone(),a[15].clone()],&[hs])?;
    }
    let reserve=Rent::get()?.minimum_balance(0);
    let rent={let d=wsol.try_borrow_data()?;require(d[109..113]==[1,0,0,0])?;u64_at(&d,113)?};
    let wrapped=wsol.lamports().checked_sub(rent).ok_or(ProgramError::InvalidAccountData)?;
    if holder.lamports().saturating_sub(reserve)==0 && wrapped==0 {msg!("STONKWHEEL_EMPTY");return Ok(());}
    invoke_signed(&Instruction {program_id:TOKEN,data:vec![9],accounts:vec![AccountMeta::new(*wsol.key,false),AccountMeta::new(*holder.key,false),AccountMeta::new_readonly(*holder.key,true)]},
        &[wsol.clone(),holder.clone(),a[15].clone()],&[hs])?;
    create_ata(holder,wsol,holder,&a[12],&a[15],&a[14],&a[17],hs)?;
    let received=holder.lamports().checked_sub(reserve).ok_or(ProgramError::InsufficientFunds)?;
    let payment=received/10;let spend=received-payment;
    require(spend>0)?;
    let before=u64_at(&bought.try_borrow_data()?,64)?;
    // Route the payment through our owned statistics account, then credit the
    // recipient directly. An operations wallet becoming a nonce/program account
    // must not give its owner a way to disable permissionless Treasury execution.
    if payment>0 {
        transfer_sol(holder,&a[22],&a[14],payment,hs)?;
        let destination=a[5].lamports().checked_add(payment).ok_or(ProgramError::ArithmeticOverflow)?;
        let remaining=a[22].lamports().checked_sub(payment).ok_or(ProgramError::InsufficientFunds)?;
        **a[22].try_borrow_mut_lamports()?=remaining;
        **a[5].try_borrow_mut_lamports()?=destination;
    }
    transfer_sol(holder,wsol,&a[14],spend,hs)?;
    invoke(&Instruction {program_id:TOKEN,data:vec![17],accounts:vec![AccountMeta::new(*wsol.key,false)]},&[wsol.clone(),a[15].clone()])?;
    let mut data=vec![143,190,90,218,196,30,51,222];data.extend_from_slice(&spend.to_le_bytes());data.extend_from_slice(&0u64.to_le_bytes());
    invoke_signed(&Instruction {program_id:CPMM,data,accounts:vec![AccountMeta::new_readonly(*holder.key,true),AccountMeta::new_readonly(*a[7].key,false),
        AccountMeta::new_readonly(c.cp_config,false),AccountMeta::new(*a[6].key,false),AccountMeta::new(*wsol.key,false),AccountMeta::new(*bought.key,false),
        AccountMeta::new(*a[9].key,false),AccountMeta::new(*a[10].key,false),AccountMeta::new_readonly(TOKEN,false),AccountMeta::new_readonly(TOKEN22,false),
        AccountMeta::new_readonly(WSOL,false),AccountMeta::new_readonly(c.mint,false),AccountMeta::new(*a[11].key,false)]},
        &[holder.clone(),a[7].clone(),a[8].clone(),a[6].clone(),wsol.clone(),bought.clone(),a[9].clone(),a[10].clone(),a[15].clone(),a[16].clone(),a[12].clone(),a[13].clone(),a[11].clone(),a[18].clone()],&[hs])?;
    let burned=u64_at(&bought.try_borrow_data()?,64)?.checked_sub(before).ok_or(ProgramError::InvalidAccountData)?;require(burned>0)?;
    let mut data=vec![15];data.extend_from_slice(&burned.to_le_bytes());data.push(6);
    invoke_signed(&Instruction {program_id:TOKEN22,data,accounts:vec![AccountMeta::new(*bought.key,false),AccountMeta::new(c.mint,false),AccountMeta::new_readonly(*burn.key,true)]},
        &[bought.clone(),a[13].clone(),burn.clone(),a[16].clone()],&[bs])?;
    require(u64_at(&wsol.try_borrow_data()?,64)?==0 && u64_at(&bought.try_borrow_data()?,64)?==before && holder.lamports()==reserve)?;
    let clock=Clock::get()?;let mut d=a[22].try_borrow_mut_data()?;
    for (offset,amount) in [(40,spend),(56,payment),(72,burned)] {
        let old=u128::from_le_bytes(d[offset..offset+16].try_into().unwrap());
        let new=old.checked_add(amount as u128).ok_or(ProgramError::ArithmeticOverflow)?;d[offset..offset+16].copy_from_slice(&new.to_le_bytes());
    }
    let count=u64_at(&d,88)?.checked_add(1).ok_or(ProgramError::ArithmeticOverflow)?;
    d[88..96].copy_from_slice(&count.to_le_bytes());d[96..104].copy_from_slice(&clock.slot.to_le_bytes());d[104..112].copy_from_slice(&clock.unix_timestamp.to_le_bytes());
    msg!("STONKWHEEL_CYCLE_V1 seq={} received={} operations={} buyback={} burned={} slot={} timestamp={}",count,received,payment,spend,burned,clock.slot,clock.unix_timestamp);
    Ok(())
}
// Before LaunchLab creates the mint, return ALL adversarial dust to the deployer.
// Both the configured mint key and deployer sign. This cannot touch token accounts
// or an active mint and removes the RPC-read-to-execution prefunding race.
fn clean_mint(program:&Pubkey,a:&[AccountInfo])->ProgramResult {
    require(a.len()==4 && a[0].is_signer && a[2].is_signer)?;
    let c=config(&a[1],program)?;require(!c.active)?;
    key(&a[0],&c.operations)?;key(&a[2],&c.mint)?;
    system_wallet(&a[0])?;system_wallet(&a[2])?;executable(&a[3],&system_program::id())?;
    let amount=a[2].lamports();
    if amount>0 { invoke(&system_instruction::transfer(a[2].key,a[0].key,amount),&[a[2].clone(),a[0].clone(),a[3].clone()])?; }
    Ok(())
}
pub fn process_instruction(program:&Pubkey,a:&[AccountInfo],data:&[u8])->ProgramResult {
    match data { [0]=>token_setup(program,a),[1]=>execute(program,a),[2]=>activate(program,a),[4]=>clean_mint(program,a),
        [3,nonce,rest @ ..] if rest.len()==32=>prepare(program,a,*nonce,Pubkey::new_from_array(rest.try_into().unwrap())),
        _=>Err(ProgramError::InvalidInstructionData) }
}
