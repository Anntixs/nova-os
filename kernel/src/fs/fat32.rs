use alloc::{string::{String,ToString},vec::Vec};
use crate::block;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Error {Io,InvalidVolume,Corrupt,NotFound,NotDirectory,IsDirectory,TooLarge,Exists,NoSpace,UnsupportedName,NotEmpty}
impl From<block::Error> for Error {fn from(_:block::Error)->Self {Self::Io}}
#[derive(Clone,Debug)]
pub struct Entry {pub name:String,pub cluster:u32,pub size:u32,pub directory:bool}
#[derive(Clone,Copy)]
pub struct Volume {pub sectors:u32,pub sectors_per_cluster:u8,pub fat_start:u32,pub fat_sectors:u32,pub fat_count:u8,pub data_start:u32,pub root:u32,pub cluster_count:u32}
fn le16(b:&[u8],i:usize)->u16 {u16::from_le_bytes([b[i],b[i+1]])}
fn le32(b:&[u8],i:usize)->u32 {u32::from_le_bytes([b[i],b[i+1],b[i+2],b[i+3]])}
impl Volume {
 pub fn mount()->Result<Self,Error> {
  let mut b=[0u8;512];block::read(0,&mut b)?;
  if b[510]!=0x55||b[511]!=0xaa||le16(&b,11)!=512||b[16]==0||b[17]!=0||b[22]!=0 {return Err(Error::InvalidVolume);}
  let spc=b[13];if spc==0||!spc.is_power_of_two()||spc>128 {return Err(Error::InvalidVolume);}
  let reserved=le16(&b,14) as u32;let fats=b[16] as u32;let fat_sectors=le32(&b,36);let sectors=le32(&b,32);let root=le32(&b,44);
  let data_start=reserved.checked_add(fats.checked_mul(fat_sectors).ok_or(Error::InvalidVolume)?).ok_or(Error::InvalidVolume)?;
  if fat_sectors==0||data_start>=sectors {return Err(Error::InvalidVolume);}
  let cluster_count=(sectors-data_start)/spc as u32;
  if cluster_count<65525||cluster_count>0x0fffffef||root<2||root>=cluster_count+2 {return Err(Error::InvalidVolume);}
  let capacity=block::capacity()?;if capacity.1!=512||sectors as u64>capacity.0 {return Err(Error::InvalidVolume);}
  Ok(Self{sectors,sectors_per_cluster:spc,fat_start:reserved,fat_sectors,fat_count:b[16],data_start,root,cluster_count})
 }
 fn valid_cluster(&self,n:u32)->bool {n>=2&&n<self.cluster_count+2}
 fn next(&self,n:u32)->Result<Option<u32>,Error> {
  if !self.valid_cluster(n){return Err(Error::Corrupt);}
  let offset=n.checked_mul(4).ok_or(Error::Corrupt)?;if offset/512>=self.fat_sectors{return Err(Error::Corrupt);}
  let mut b=[0u8;512];block::read((self.fat_start+offset/512) as u64,&mut b)?;
  let value=le32(&b,(offset%512) as usize)&0x0fff_ffff;
  if value>=0x0fff_fff8 {return Ok(None);}
  if !self.valid_cluster(value) {return Err(Error::Corrupt);}
  Ok(Some(value))
 }
 fn sector(&self,n:u32)->Result<u32,Error> {
  if !self.valid_cluster(n){return Err(Error::Corrupt);}
  let lba=self.data_start.checked_add((n-2).checked_mul(self.sectors_per_cluster as u32).ok_or(Error::Corrupt)?).ok_or(Error::Corrupt)?;
  if lba>=self.sectors{return Err(Error::Corrupt);}Ok(lba)
 }
 pub fn read_file(&self,e:&Entry)->Result<Vec<u8>,Error> {
  if e.directory {return Err(Error::IsDirectory);}
  if e.size>16*1024*1024{return Err(Error::TooLarge);}
  let mut out=Vec::new();out.try_reserve_exact(e.size as usize).map_err(|_|Error::TooLarge)?;
  if e.size==0{return Ok(out);}
  let mut current=e.cluster;let mut traversed=0u32;
  while out.len()<e.size as usize {
   if traversed>=self.cluster_count{return Err(Error::Corrupt);}
   let base=self.sector(current)?;
   for s in 0..self.sectors_per_cluster as u32 {
    let mut b=[0u8;512];block::read((base+s) as u64,&mut b)?;
    let take=(e.size as usize-out.len()).min(512);out.extend_from_slice(&b[..take]);
    if out.len()==e.size as usize {return Ok(out);}
   }
   current=self.next(current)?.ok_or(Error::Corrupt)?;traversed+=1;
  }
  Ok(out)
 }
 pub fn entries(&self,start:u32)->Result<Vec<Entry>,Error> {
  let mut out=Vec::new();let mut current=start;let mut traversed=0u32;
  let mut lfn=[0u16;260];let mut lfn_active=false;let mut checksum=0u8;
  loop {
   if traversed>=self.cluster_count{return Err(Error::Corrupt);}
   let base=self.sector(current)?;
   for s in 0..self.sectors_per_cluster as u32 {
    let mut b=[0u8;512];block::read((base+s) as u64,&mut b)?;
    for raw in b.chunks_exact(32) {
     if raw[0]==0 {return Ok(out);}
     if raw[0]==0xe5 {lfn_active=false;continue;}
     if raw[11]==15 {
      let ord=(raw[0]&0x1f) as usize;
      if ord==0||ord>20 {lfn_active=false;continue;}
      if raw[0]&0x40!=0 {lfn=[0xffff;260];lfn_active=true;checksum=raw[13];}
      if !lfn_active||checksum!=raw[13]{continue;}
      let offsets=[1,3,5,7,9,14,16,18,20,22,24,28,30];
      for (i,offset) in offsets.iter().enumerate(){lfn[(ord-1)*13+i]=le16(raw,*offset);}
      continue;
     }
     if raw[11]&8!=0 {lfn_active=false;continue;}
     let mut sum=0u8;for byte in &raw[..11]{sum=sum.rotate_right(1).wrapping_add(*byte);}
     let name=if lfn_active&&sum==checksum {
      let end=lfn.iter().position(|&x|x==0||x==0xffff).unwrap_or(260);
      String::from_utf16(&lfn[..end]).map_err(|_|Error::Corrupt)?
     } else {
      let stem=core::str::from_utf8(&raw[..8]).map_err(|_|Error::Corrupt)?.trim_end();
      let ext=core::str::from_utf8(&raw[8..11]).map_err(|_|Error::Corrupt)?.trim_end();
      let stem=if raw[12]&0x08!=0{stem.to_ascii_lowercase()}else{stem.to_string()};
      let ext=if raw[12]&0x10!=0{ext.to_ascii_lowercase()}else{ext.to_string()};
      if ext.is_empty(){stem}else {alloc::format!("{}.{}",stem,ext)}
     };
     lfn_active=false;
     let cluster=((le16(raw,20) as u32)<<16)|le16(raw,26) as u32;
     out.push(Entry{name,cluster,size:le32(raw,28),directory:raw[11]&0x10!=0});
    }
   }
   current=match self.next(current)? {Some(n)=>n,None=>return Ok(out)};traversed+=1;
  }
 }
}

impl Volume {
 fn set_fat(&self,n:u32,value:u32)->Result<(),Error>{
  if !self.valid_cluster(n){return Err(Error::Corrupt)}
  let offset=n.checked_mul(4).ok_or(Error::Corrupt)?;if offset/512>=self.fat_sectors{return Err(Error::Corrupt)}
  for copy in 0..self.fat_count as u32 {
   let lba=self.fat_start+copy*self.fat_sectors+offset/512;
   let mut sector=[0u8;512];block::read(lba as u64,&mut sector)?;
   let old=le32(&sector,(offset%512) as usize);
   sector[(offset%512) as usize..][..4].copy_from_slice(&((old&0xf000_0000)|(value&0x0fff_ffff)).to_le_bytes());
   block::write(lba as u64,&sector)?;
  }
  Ok(())
 }
 fn alloc_cluster(&self)->Result<u32,Error>{
  let mut sector=[0u8;512];let mut cached=u32::MAX;
  for n in 2..self.cluster_count+2 {
   let fat_sector=n/128;
   if fat_sector>=self.fat_sectors{return Err(Error::Corrupt)}
   if fat_sector!=cached {block::read((self.fat_start+fat_sector) as u64,&mut sector)?;cached=fat_sector;}
   if le32(&sector,(n%128*4) as usize)&0x0fff_ffff==0 {
    self.set_fat(n,0x0fff_ffff)?;
    let zero=[0u8;512];let base=self.sector(n)?;
    for i in 0..self.sectors_per_cluster as u32 {block::write((base+i) as u64,&zero)?;}
    return Ok(n)
   }
  }
  Err(Error::NoSpace)
 }
 fn free_chain(&self,start:u32)->Result<(),Error>{
  if start==0{return Ok(())}
  let mut n=start;
  for _ in 0..self.cluster_count {let next=self.next(n)?;self.set_fat(n,0)?;match next {Some(v)=>n=v,None=>return Ok(())}}
  Err(Error::Corrupt)
 }
 fn short(name:&str)->Result<[u8;11],Error>{
  let (stem,ext)=name.split_once('.').unwrap_or((name,""));
  if stem.is_empty()||stem.len()>8||ext.len()>3||stem=="."||stem==".." {return Err(Error::UnsupportedName)}
  let mut key=[b' ';11];
  for (i,b) in stem.bytes().enumerate(){if !b.is_ascii_alphanumeric()&&b!=b'_'&&b!=b'-'{return Err(Error::UnsupportedName)}key[i]=b.to_ascii_uppercase();}
  for (i,b) in ext.bytes().enumerate(){if !b.is_ascii_alphanumeric()&&b!=b'_'&&b!=b'-'{return Err(Error::UnsupportedName)}key[8+i]=b.to_ascii_uppercase();}
  Ok(key)
 }
 fn locate_short(&self,parent:u32,key:&[u8;11])->Result<Option<(u32,usize,[u8;512])>,Error>{
  let mut n=parent;
  for _ in 0..self.cluster_count {
   let base=self.sector(n)?;
   for i in 0..self.sectors_per_cluster as u32 {
    let lba=base+i;let mut sector=[0u8;512];block::read(lba as u64,&mut sector)?;
    for off in (0..512).step_by(32) {let raw=&sector[off..off+32];if raw[0]==0{return Ok(None)}
     if raw[0]!=0xe5&&raw[11]!=15&&raw[0..11]==*key {return Ok(Some((lba,off,sector)))} }
   }
   n=match self.next(n)?{Some(x)=>x,None=>return Ok(None)};
  }Err(Error::Corrupt)
 }
 fn empty_slot(&self,parent:u32)->Result<(u32,usize,[u8;512]),Error>{
  let mut n=parent;
  for _ in 0..self.cluster_count {
   let base=self.sector(n)?;
   for i in 0..self.sectors_per_cluster as u32 {
    let lba=base+i;let mut sector=[0u8;512];block::read(lba as u64,&mut sector)?;
    for off in (0..512).step_by(32){if sector[off]==0||sector[off]==0xe5{return Ok((lba,off,sector))}}
   }
   match self.next(n)? {Some(x)=>n=x,None=>{
    let new=self.alloc_cluster()?;self.set_fat(n,new)?;return Ok((self.sector(new)?,0,[0u8;512]))
   }}
  }Err(Error::Corrupt)
 }
 fn write_record(&self,lba:u32,off:usize,mut sector:[u8;512],key:[u8;11],cluster:u32,size:u32,dir:bool)->Result<(),Error>{
  let e=&mut sector[off..off+32];e.fill(0);e[..11].copy_from_slice(&key);e[11]=if dir{0x10}else{0x20};
  e[20..22].copy_from_slice(&((cluster>>16) as u16).to_le_bytes());e[26..28].copy_from_slice(&(cluster as u16).to_le_bytes());e[28..32].copy_from_slice(&size.to_le_bytes());block::write(lba as u64,&sector)?;Ok(())
 }
 pub fn create(&self,parent:u32,name:&str,dir:bool)->Result<(),Error>{
  let key=Self::short(name)?;
  if self.locate_short(parent,&key)?.is_some(){return Err(Error::Exists)}
  let (lba,off,sector)=self.empty_slot(parent)?;
  let cluster=if dir{self.alloc_cluster()?}else{0};
  if dir {
   let mut body=[0u8;512];let mut dot=[b' ';11];dot[0]=b'.';
   let mut dotdot=[b' ';11];dotdot[0]=b'.';dotdot[1]=b'.';
   let first=self.sector(cluster)?;
   self.write_record(first,0,body,dot,cluster,0,true)?;
   block::read(first as u64,&mut body)?;
   self.write_record(first,32,body,dotdot,parent,0,true)?;
  }
  self.write_record(lba,off,sector,key,cluster,0,dir)
 }
 pub fn remove(&self,parent:u32,name:&str)->Result<(),Error>{
  let key=Self::short(name)?;let (lba,off,mut sector)=self.locate_short(parent,&key)?.ok_or(Error::NotFound)?;
  let raw=&sector[off..off+32];let cluster=((le16(raw,20) as u32)<<16)|le16(raw,26) as u32;
  if raw[11]&0x10!=0 && self.entries(cluster)?.iter().any(|e|e.name!="."&&e.name!="..") {return Err(Error::NotEmpty)}
  sector[off]=0xe5;block::write(lba as u64,&sector)?;self.free_chain(cluster)
 }
 pub fn write_file(&self,parent:u32,name:&str,offset:usize,data:&[u8],truncate:bool)->Result<usize,Error>{
  let key=Self::short(name)?;let (lba,off,mut record)=self.locate_short(parent,&key)?.ok_or(Error::NotFound)?;
  if record[off+11]&0x10!=0{return Err(Error::IsDirectory)}
  let mut first=((le16(&record,off+20) as u32)<<16)|le16(&record,off+26) as u32;
  let old_size=le32(&record,off+28) as usize;
  let start=if truncate{0}else{offset};
  let end=start.checked_add(data.len()).filter(|&n|n<=16*1024*1024).ok_or(Error::TooLarge)?;
  if truncate {self.free_chain(first)?;first=0;record[off+20..off+22].fill(0);record[off+26..off+28].fill(0);record[off+28..off+32].fill(0);block::write(lba as u64,&record)?;}
  if end==0 {return Ok(0)}
  let cluster_bytes=self.sectors_per_cluster as usize*512;
  let needed=(end+cluster_bytes-1)/cluster_bytes;
  let mut chain=Vec::new();
  if first!=0 {let mut n=first;for _ in 0..self.cluster_count {chain.push(n);match self.next(n)?{Some(x)=>n=x,None=>break}}}
  while chain.len()<needed {
   let n=self.alloc_cluster()?;
   if let Some(&last)=chain.last(){self.set_fat(last,n)?}else{first=n;}
   chain.push(n);
  }
  // Preserve FAT consistency: publish the new first cluster before writing data.
  record[off+20..off+22].copy_from_slice(&((first>>16) as u16).to_le_bytes());record[off+26..off+28].copy_from_slice(&(first as u16).to_le_bytes());
  block::write(lba as u64,&record)?;
  // Holes are zero filled, including bytes in an already allocated cluster.
  let zero=[0u8;512];let mut position=if truncate{0}else{old_size};
  while position<start {let index=position/cluster_bytes;let sector_index=(position%cluster_bytes)/512;let lba_data=self.sector(chain[index])?+sector_index as u32;let mut buf=[0u8;512];block::read(lba_data as u64,&mut buf)?;
   let within=position%512;let take=(start-position).min(512-within);buf[within..within+take].fill(0);block::write(lba_data as u64,&buf)?;position+=take;}
  let mut written=0usize;
  while written<data.len(){let pos=start+written;let index=pos/cluster_bytes;let sector_index=(pos%cluster_bytes)/512;let lba_data=self.sector(chain[index])?+sector_index as u32;
   let mut buf=zero;block::read(lba_data as u64,&mut buf)?;let within=pos%512;let take=(data.len()-written).min(512-within);buf[within..within+take].copy_from_slice(&data[written..written+take]);block::write(lba_data as u64,&buf)?;written+=take;}
  let size=if truncate{end}else{old_size.max(end)};record[off+28..off+32].copy_from_slice(&(size as u32).to_le_bytes());block::write(lba as u64,&record)?;Ok(written)
 }
}
